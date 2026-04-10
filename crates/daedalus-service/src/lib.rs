use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use daedalus_config::{AppConfig, ConfigStore, ResolvedConfig};
use daedalus_core::{DaedalusError, Result, looks_like_model_file, normalize_path, now_utc};
use daedalus_db::{CatalogDb, LibraryItemInsert};
use daedalus_domain::{
    DownloadRequest, ImportRequest, Job, JobKind, JobStatus, LibraryItem, ModelKind, ServiceHealth,
    SourceInfo, SourceKind, SourceRef,
};
use daedalus_downloads::{PlacementInput, compute_managed_path};
use daedalus_sources::CivitaiAdapter;
use tracing::info;
use walkdir::WalkDir;

#[derive(Clone)]
pub struct DaedalusService {
    config_store: ConfigStore,
    config: Arc<RwLock<AppConfig>>,
    db: Arc<RwLock<CatalogDb>>,
    runtime_mode: Arc<String>,
}

impl DaedalusService {
    pub fn bootstrap_default(runtime_mode: impl Into<String>) -> Result<Self> {
        Self::from_store(ConfigStore::discover(), runtime_mode)
    }

    pub fn from_store(config_store: ConfigStore, runtime_mode: impl Into<String>) -> Result<Self> {
        let config = config_store.load_or_default()?;
        let report = config.validate()?;
        if !report.is_ok() {
            return Err(DaedalusError::Validation(report.errors.join("; ")));
        }

        let resolved = config.resolved()?;
        resolved.ensure_directories()?;
        let db = CatalogDb::new(resolved.database_path.clone())?;

        Ok(Self {
            config_store,
            config: Arc::new(RwLock::new(config)),
            db: Arc::new(RwLock::new(db)),
            runtime_mode: Arc::new(runtime_mode.into()),
        })
    }

    pub fn config_store(&self) -> &ConfigStore {
        &self.config_store
    }

    pub fn current_config(&self) -> Result<AppConfig> {
        Ok(self
            .config
            .read()
            .map_err(|_| DaedalusError::Other("config lock poisoned".to_string()))?
            .clone())
    }

    pub fn update_config(&self, config: AppConfig) -> Result<()> {
        let report = config.validate()?;
        if !report.is_ok() {
            return Err(DaedalusError::Validation(report.errors.join("; ")));
        }

        let resolved = config.resolved()?;
        resolved.ensure_directories()?;
        let db = CatalogDb::new(resolved.database_path.clone())?;
        self.config_store.save(&config)?;

        *self
            .config
            .write()
            .map_err(|_| DaedalusError::Other("config lock poisoned".to_string()))? = config;
        *self
            .db
            .write()
            .map_err(|_| DaedalusError::Other("db lock poisoned".to_string()))? = db;

        Ok(())
    }

    pub fn health(&self) -> Result<ServiceHealth> {
        let db = self.db()?;
        Ok(ServiceHealth {
            status: "ok".to_string(),
            mode: (*self.runtime_mode).clone(),
            library_item_count: db.library_item_count()?,
            source_count: self.list_sources()?.len(),
        })
    }

    pub fn list_sources(&self) -> Result<Vec<SourceInfo>> {
        let config = self.current_config()?;
        let civitai = CivitaiAdapter::new(
            config.sources.civitai.api_base_url,
            config.sources.civitai.web_base_url,
            config.sources.civitai.enabled,
        );
        Ok(vec![civitai.info()])
    }

    pub fn list_library_items(&self) -> Result<Vec<LibraryItem>> {
        self.db()?.list_library_items()
    }

    pub fn get_library_item(&self, id: i64) -> Result<Option<LibraryItem>> {
        self.db()?.get_library_item(id)
    }

    pub fn import_local_file(&self, request: ImportRequest) -> Result<LibraryItem> {
        let source_path = normalize_path(Path::new(&request.path));
        if !source_path.exists() || !source_path.is_file() {
            return Err(DaedalusError::NotFound(format!(
                "local file does not exist: {}",
                source_path.display()
            )));
        }

        let resolved = self.resolved_config()?;
        let kind = request
            .kind
            .or_else(|| self.infer_kind_from_path(&source_path).ok())
            .unwrap_or(ModelKind::Other);

        let storage_path = if request.copy_into_library {
            let file_name = source_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| DaedalusError::Validation("source file must have a valid filename".to_string()))?;
            let display_name = request.display_name.clone().unwrap_or_else(|| {
                source_path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .unwrap_or("imported-model")
                    .to_string()
            });
            let destination = unique_destination(compute_managed_path(PlacementInput {
                root: &resolved.default_storage_root,
                kind,
                model_name: &display_name,
                version_name: "manual-import",
                filename: file_name,
            }));
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(&source_path, &destination)?;
            destination
        } else {
            source_path.clone()
        };

        let display_name = request.display_name.unwrap_or_else(|| {
            storage_path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("imported-model")
                .to_string()
        });

        let item = self.db()?.upsert_library_item(&LibraryItemInsert {
            display_name,
            primary_model_kind: kind,
            source: Some(SourceRef {
                source_kind: SourceKind::Local,
                source_model_id: None,
                source_version_id: None,
                source_url: None,
                source_category: None,
            }),
            installed_version: None,
            storage_path: storage_path.display().to_string(),
            favorite: false,
            pinned: false,
            notes: None,
            last_verified_at: Some(now_utc()),
        })?;

        info!("imported library item {}", item.storage_path);
        Ok(item)
    }

    pub fn rescan_library(&self) -> Result<Job> {
        let db = self.db()?;
        let mut job = db.create_job(JobKind::Rescan, "Rescanning configured model directories")?;
        let _ = db.update_job_status(job.id, JobStatus::Running, 0.05, None)?;

        let resolved = self.resolved_config()?;
        let mut scanned = 0usize;
        for (kind, root) in &resolved.model_paths {
            for entry in WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(std::result::Result::ok)
            {
                let path = entry.path();
                if !path.is_file() || !looks_like_model_file(path) {
                    continue;
                }

                scanned += 1;
                db.upsert_library_item(&LibraryItemInsert {
                    display_name: path
                        .file_stem()
                        .and_then(|stem| stem.to_str())
                        .unwrap_or("discovered-model")
                        .to_string(),
                    primary_model_kind: *kind,
                    source: Some(SourceRef {
                        source_kind: SourceKind::Local,
                        source_model_id: None,
                        source_version_id: None,
                        source_url: None,
                        source_category: None,
                    }),
                    installed_version: None,
                    storage_path: normalize_path(path).display().to_string(),
                    favorite: false,
                    pinned: false,
                    notes: None,
                    last_verified_at: Some(now_utc()),
                })?;
            }
        }

        let summary = format!("Rescanned configured model directories ({scanned} files)");
        let _ = db.update_job_status(job.id, JobStatus::Completed, 1.0, None)?;
        job = db
            .get_job(job.id)?
            .ok_or_else(|| DaedalusError::Database("rescan job disappeared".to_string()))?;
        job.summary = summary;
        Ok(job)
    }

    pub fn queue_download(&self, request: DownloadRequest) -> Result<Job> {
        self.db()?.create_job(
            JobKind::Download,
            &format!(
                "Queued {} download for source file {}",
                request.model_kind.label(),
                request.source_file_id
            ),
        )
    }

    pub fn cancel_job(&self, id: i64) -> Result<Option<Job>> {
        self.db()?.update_job_status(id, JobStatus::Cancelled, 0.0, None)
    }

    pub fn list_jobs(&self) -> Result<Vec<Job>> {
        self.db()?.list_jobs()
    }

    pub fn get_job(&self, id: i64) -> Result<Option<Job>> {
        self.db()?.get_job(id)
    }

    pub fn resolved_config(&self) -> Result<ResolvedConfig> {
        self.current_config()?.resolved()
    }

    pub fn infer_kind_from_path(&self, path: &Path) -> Result<ModelKind> {
        let resolved = self.resolved_config()?;
        for (kind, root) in &resolved.model_paths {
            if path.starts_with(root) {
                return Ok(*kind);
            }
        }

        let kind = match path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref()
        {
            Some("json") | Some("yaml") | Some("yml") => ModelKind::Workflows,
            Some("txt") => ModelKind::Wildcards,
            Some("vae") => ModelKind::Vae,
            Some("safetensors") | Some("ckpt") | Some("pt") | Some("pth") | Some("bin") => {
                ModelKind::Checkpoint
            }
            _ => ModelKind::Other,
        };

        Ok(kind)
    }

    fn db(&self) -> Result<CatalogDb> {
        Ok(self
            .db
            .read()
            .map_err(|_| DaedalusError::Other("db lock poisoned".to_string()))?
            .clone())
    }
}

fn unique_destination(mut destination: PathBuf) -> PathBuf {
    if !destination.exists() {
        return destination;
    }

    let stem = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("file")
        .to_string();
    let extension = destination
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let parent = destination.parent().map(Path::to_path_buf).unwrap_or_default();

    for index in 1.. {
        let candidate = parent.join(format!("{stem}-{index}{extension}"));
        if !candidate.exists() {
            destination = candidate;
            break;
        }
    }

    destination
}

#[cfg(test)]
mod tests {
    use super::*;
    use daedalus_config::{DatabaseConfig, GuiMode, LibraryConfig, ModelPathConfig};
    use tempfile::tempdir;

    fn test_config(root: &Path) -> AppConfig {
        let root_str = root.display().to_string();
        AppConfig {
            version: 1,
            library: LibraryConfig {
                default_storage_root: root_str.clone(),
                temp_download_root: format!("{root_str}/.tmp"),
                preview_cache_root: format!("{root_str}/.previews"),
                managed_by_kind: true,
                deduplicate_by_sha256: true,
            },
            database: DatabaseConfig {
                path: format!("{root_str}/catalog.db"),
            },
            daemon: daedalus_config::DaemonConfig {
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 4590,
            },
            gui: daedalus_config::GuiConfig {
                default_mode: GuiMode::Embedded,
                remote_url: "http://127.0.0.1:4590".to_string(),
            },
            sources: daedalus_config::SourcesConfig {
                civitai: daedalus_config::CivitaiConfig {
                    enabled: true,
                    api_base_url: "https://civitai.com/api/v1".to_string(),
                    web_base_url: "https://civitai.com".to_string(),
                    api_token: String::new(),
                    sync_preview_images: true,
                    sync_creator_metadata: true,
                },
            },
            model_paths: ModelPathConfig::defaults(&root_str),
        }
    }

    #[test]
    fn imports_local_file() {
        let temp = tempdir().expect("tempdir");
        let config_path = temp.path().join("config.toml");
        let store = ConfigStore::new(&config_path);
        let config = test_config(temp.path());
        store.save(&config).expect("save config");
        let service = DaedalusService::from_store(store, "test").expect("service");

        let source_file = temp.path().join("existing.safetensors");
        fs::write(&source_file, b"weights").expect("write source");

        let item = service
            .import_local_file(ImportRequest {
                path: source_file.display().to_string(),
                display_name: Some("Existing".to_string()),
                kind: Some(ModelKind::Checkpoint),
                copy_into_library: false,
            })
            .expect("import");

        assert_eq!(item.display_name, "Existing");
        assert!(item.storage_path.ends_with("existing.safetensors"));
    }
}
