use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use daedalus_core::{DaedalusError, Result};
use daedalus_domain::ModelKind;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppConfig {
    pub version: u32,
    pub library: LibraryConfig,
    pub database: DatabaseConfig,
    pub daemon: DaemonConfig,
    pub gui: GuiConfig,
    pub sources: SourcesConfig,
    pub model_paths: ModelPathConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        let platform = PlatformPaths::detect();
        let models_root = platform.data_dir.join("models");
        let models_root_string = models_root.display().to_string();

        Self {
            version: CONFIG_VERSION,
            library: LibraryConfig {
                default_storage_root: models_root_string.clone(),
                temp_download_root: models_root.join(".tmp").display().to_string(),
                preview_cache_root: platform.cache_dir.join("previews").display().to_string(),
                managed_by_kind: true,
                deduplicate_by_sha256: true,
            },
            database: DatabaseConfig {
                path: platform.data_dir.join("daedalus.db").display().to_string(),
            },
            daemon: DaemonConfig {
                enabled: true,
                host: "127.0.0.1".to_string(),
                port: 4590,
            },
            gui: GuiConfig {
                default_mode: GuiMode::Auto,
                remote_url: "http://127.0.0.1:4590".to_string(),
            },
            sources: SourcesConfig {
                civitai: CivitaiConfig {
                    enabled: true,
                    api_base_url: "https://civitai.com/api/v1".to_string(),
                    web_base_url: "https://civitai.com".to_string(),
                    api_token: String::new(),
                    sync_preview_images: true,
                    sync_creator_metadata: true,
                },
            },
            model_paths: ModelPathConfig::defaults(&models_root_string),
        }
    }
}

impl AppConfig {
    pub fn resolved(&self) -> Result<ResolvedConfig> {
        let platform = PlatformPaths::detect();
        let storage_root = resolve_path(&self.library.default_storage_root, &platform.data_dir)?;
        let temp_download_root = resolve_against_root(&storage_root, &self.library.temp_download_root)?;
        let preview_cache_root = resolve_path(&self.library.preview_cache_root, &platform.cache_dir)?;
        let database_path = resolve_path(&self.database.path, &platform.data_dir)?;

        let mut model_paths = BTreeMap::new();
        for (kind, raw) in self.model_paths.iter() {
            model_paths.insert(kind, resolve_against_root(&storage_root, raw)?);
        }

        Ok(ResolvedConfig {
            config_dir: platform.config_dir,
            cache_dir: platform.cache_dir,
            data_dir: platform.data_dir,
            default_storage_root: storage_root,
            temp_download_root,
            preview_cache_root,
            database_path,
            model_paths,
        })
    }

    pub fn validate(&self) -> Result<ValidationReport> {
        let resolved = self.resolved()?;
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if self.version != CONFIG_VERSION {
            errors.push(format!(
                "unsupported config version {}; expected {}",
                self.version, CONFIG_VERSION
            ));
        }

        if self.daemon.host.trim().is_empty() {
            errors.push("daemon.host must not be empty".to_string());
        }

        if self.gui.remote_url.trim().is_empty() {
            errors.push("gui.remote_url must not be empty".to_string());
        }

        let mut seen = BTreeSet::new();
        for (kind, path) in &resolved.model_paths {
            if !seen.insert(path.clone()) {
                errors.push(format!(
                    "model path for {} resolves to a duplicate directory: {}",
                    kind.label(),
                    path.display()
                ));
            }
            if !path.exists() {
                warnings.push(format!(
                    "model path for {} does not exist yet: {}",
                    kind.label(),
                    path.display()
                ));
            }
        }

        if !resolved.default_storage_root.exists() {
            warnings.push(format!(
                "default storage root does not exist yet: {}",
                resolved.default_storage_root.display()
            ));
        }

        if let Some(parent) = resolved.database_path.parent() {
            if !parent.exists() {
                warnings.push(format!("database directory does not exist yet: {}", parent.display()));
            }
        }

        Ok(ValidationReport { errors, warnings })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryConfig {
    pub default_storage_root: String,
    pub temp_download_root: String,
    pub preview_cache_root: String,
    pub managed_by_kind: bool,
    pub deduplicate_by_sha256: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuiConfig {
    pub default_mode: GuiMode,
    pub remote_url: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GuiMode {
    Auto,
    Embedded,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourcesConfig {
    pub civitai: CivitaiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CivitaiConfig {
    pub enabled: bool,
    pub api_base_url: String,
    pub web_base_url: String,
    pub api_token: String,
    pub sync_preview_images: bool,
    pub sync_creator_metadata: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelPathConfig {
    pub checkpoint: String,
    pub embedding: String,
    pub hypernetwork: String,
    pub aesthetic_gradient: String,
    pub lora: String,
    pub lycoris: String,
    pub dora: String,
    pub controlnet: String,
    pub upscaler: String,
    pub motion: String,
    pub vae: String,
    pub poses: String,
    pub wildcards: String,
    pub workflows: String,
    pub detection: String,
    pub other: String,
}

impl ModelPathConfig {
    pub fn defaults(root: &str) -> Self {
        Self {
            checkpoint: format!("{root}/checkpoints"),
            embedding: format!("{root}/embeddings"),
            hypernetwork: format!("{root}/hypernetworks"),
            aesthetic_gradient: format!("{root}/aesthetic-gradients"),
            lora: format!("{root}/loras"),
            lycoris: format!("{root}/lycoris"),
            dora: format!("{root}/dora"),
            controlnet: format!("{root}/controlnet"),
            upscaler: format!("{root}/upscalers"),
            motion: format!("{root}/motion-modules"),
            vae: format!("{root}/vae"),
            poses: format!("{root}/poses"),
            wildcards: format!("{root}/wildcards"),
            workflows: format!("{root}/workflows"),
            detection: format!("{root}/detection"),
            other: format!("{root}/other"),
        }
    }

    pub fn iter(&self) -> Vec<(ModelKind, &str)> {
        vec![
            (ModelKind::Checkpoint, self.checkpoint.as_str()),
            (ModelKind::Embedding, self.embedding.as_str()),
            (ModelKind::Hypernetwork, self.hypernetwork.as_str()),
            (ModelKind::AestheticGradient, self.aesthetic_gradient.as_str()),
            (ModelKind::LoRA, self.lora.as_str()),
            (ModelKind::LyCORIS, self.lycoris.as_str()),
            (ModelKind::DoRA, self.dora.as_str()),
            (ModelKind::ControlNet, self.controlnet.as_str()),
            (ModelKind::Upscaler, self.upscaler.as_str()),
            (ModelKind::Motion, self.motion.as_str()),
            (ModelKind::Vae, self.vae.as_str()),
            (ModelKind::Poses, self.poses.as_str()),
            (ModelKind::Wildcards, self.wildcards.as_str()),
            (ModelKind::Workflows, self.workflows.as_str()),
            (ModelKind::Detection, self.detection.as_str()),
            (ModelKind::Other, self.other.as_str()),
        ]
    }

    pub fn get(&self, kind: ModelKind) -> &str {
        match kind {
            ModelKind::Checkpoint => &self.checkpoint,
            ModelKind::Embedding => &self.embedding,
            ModelKind::Hypernetwork => &self.hypernetwork,
            ModelKind::AestheticGradient => &self.aesthetic_gradient,
            ModelKind::LoRA => &self.lora,
            ModelKind::LyCORIS => &self.lycoris,
            ModelKind::DoRA => &self.dora,
            ModelKind::ControlNet => &self.controlnet,
            ModelKind::Upscaler => &self.upscaler,
            ModelKind::Motion => &self.motion,
            ModelKind::Vae => &self.vae,
            ModelKind::Poses => &self.poses,
            ModelKind::Wildcards => &self.wildcards,
            ModelKind::Workflows => &self.workflows,
            ModelKind::Detection => &self.detection,
            ModelKind::Other => &self.other,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformPaths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
}

impl PlatformPaths {
    pub fn detect() -> Self {
        if let Some(project_dirs) = ProjectDirs::from("", "", "daedalus") {
            return Self {
                config_dir: project_dirs.config_dir().to_path_buf(),
                cache_dir: project_dirs.cache_dir().to_path_buf(),
                data_dir: project_dirs.data_local_dir().to_path_buf(),
            };
        }

        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let home = PathBuf::from(home);
        Self {
            config_dir: home.join(".config/daedalus"),
            cache_dir: home.join(".cache/daedalus"),
            data_dir: home.join(".local/share/daedalus"),
        }
    }

    pub fn default_config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedConfig {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub data_dir: PathBuf,
    pub default_storage_root: PathBuf,
    pub temp_download_root: PathBuf,
    pub preview_cache_root: PathBuf,
    pub database_path: PathBuf,
    pub model_paths: BTreeMap<ModelKind, PathBuf>,
}

impl ResolvedConfig {
    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)?;
        fs::create_dir_all(&self.cache_dir)?;
        fs::create_dir_all(&self.data_dir)?;
        fs::create_dir_all(&self.default_storage_root)?;
        fs::create_dir_all(&self.temp_download_root)?;
        fs::create_dir_all(&self.preview_cache_root)?;
        if let Some(parent) = self.database_path.parent() {
            fs::create_dir_all(parent)?;
        }
        for path in self.model_paths.values() {
            fs::create_dir_all(path)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    path: PathBuf,
}

impl ConfigStore {
    pub fn discover() -> Self {
        let explicit = env::var("DAEDALUS_CONFIG").ok().map(PathBuf::from);
        let path = explicit.unwrap_or_else(|| PlatformPaths::detect().default_config_path());
        Self { path }
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AppConfig> {
        let raw = fs::read_to_string(&self.path)?;
        let mut config: AppConfig = toml::from_str(&raw)
            .map_err(|err| DaedalusError::Config(format!("failed to parse {}: {err}", self.path.display())))?;
        apply_env_overrides(&mut config);
        Ok(config)
    }

    pub fn load_or_default(&self) -> Result<AppConfig> {
        if self.path.exists() {
            return self.load();
        }

        let config = AppConfig::default();
        self.save(&config)?;
        Ok(config)
    }

    pub fn save(&self, config: &AppConfig) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let rendered = toml::to_string_pretty(config)
            .map_err(|err| DaedalusError::Config(format!("failed to serialize config: {err}")))?;
        fs::write(&self.path, rendered)?;
        Ok(())
    }
}

fn apply_env_overrides(config: &mut AppConfig) {
    if let Ok(token) = env::var("DAEDALUS_CIVITAI_API_TOKEN") {
        config.sources.civitai.api_token = token;
    }
    if let Ok(remote_url) = env::var("DAEDALUS_REMOTE_URL") {
        config.gui.remote_url = remote_url;
    }
}

fn resolve_path(raw: &str, relative_base: &Path) -> Result<PathBuf> {
    let expanded = shellexpand::full(raw)
        .map_err(|err| DaedalusError::Config(format!("failed to expand path '{raw}': {err}")))?;
    let path = PathBuf::from(expanded.as_ref());
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(relative_base.join(path))
    }
}

fn resolve_against_root(root: &Path, raw: &str) -> Result<PathBuf> {
    let expanded = shellexpand::full(raw)
        .map_err(|err| DaedalusError::Config(format!("failed to expand path '{raw}': {err}")))?;
    let path = PathBuf::from(expanded.as_ref());
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(root.join(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_validates() {
        let config = AppConfig::default();
        let report = config.validate().expect("validate config");
        assert!(report.is_ok(), "unexpected validation errors: {:?}", report.errors);
    }

    #[test]
    fn duplicate_model_paths_fail_validation() {
        let mut config = AppConfig::default();
        config.model_paths.embedding = config.model_paths.checkpoint.clone();
        let report = config.validate().expect("validate config");
        assert!(!report.is_ok());
        assert!(report.errors.iter().any(|msg| msg.contains("duplicate directory")));
    }
}
