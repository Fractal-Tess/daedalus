#[cfg(feature = "desktop")]
use daedalus_client::DaedalusClient;
#[cfg(feature = "desktop")]
use daedalus_config::{AppConfig, ConfigStore, GuiMode, ValidationReport};
#[cfg(feature = "desktop")]
use daedalus_core::Result;
#[cfg(feature = "desktop")]
use daedalus_domain::{Job, LibraryItem, ModelKind};
#[cfg(feature = "desktop")]
use daedalus_service::DaedalusService;

#[cfg(feature = "desktop")]
fn main() -> std::result::Result<(), eframe::Error> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Daedalus",
        options,
        Box::new(|_cc| Ok(Box::new(DaedalusApp::bootstrap()))),
    )
}

#[cfg(not(feature = "desktop"))]
fn main() {
    eprintln!("daedalus-gui was built without the `desktop` feature. Rebuild with `--features desktop` to launch the native egui shell.");
}

#[cfg(feature = "desktop")]
#[derive(Clone)]
enum Backend {
    Embedded(DaedalusService),
    Remote(DaedalusClient),
}

#[cfg(feature = "desktop")]
impl Backend {
    fn label(&self) -> &'static str {
        match self {
            Self::Embedded(_) => "Embedded",
            Self::Remote(_) => "Remote",
        }
    }

    fn current_config(&self) -> Result<AppConfig> {
        match self {
            Self::Embedded(service) => service.current_config(),
            Self::Remote(client) => client.get_config(),
        }
    }

    fn update_config(&self, config: &AppConfig) -> Result<()> {
        match self {
            Self::Embedded(service) => service.update_config(config.clone()),
            Self::Remote(client) => client.update_config(config),
        }
    }

    fn list_library_items(&self) -> Result<Vec<LibraryItem>> {
        match self {
            Self::Embedded(service) => service.list_library_items(),
            Self::Remote(client) => client.list_library_items(),
        }
    }

    fn list_jobs(&self) -> Result<Vec<Job>> {
        match self {
            Self::Embedded(service) => service.list_jobs(),
            Self::Remote(client) => client.list_jobs(),
        }
    }

    fn rescan_library(&self) -> Result<Job> {
        match self {
            Self::Embedded(service) => service.rescan_library(),
            Self::Remote(client) => client.rescan_library(),
        }
    }
}

#[cfg(feature = "desktop")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Library,
    Jobs,
    Settings,
}

#[cfg(feature = "desktop")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum LibrarySort {
    Recent,
    Name,
    Kind,
    Path,
}

#[cfg(feature = "desktop")]
struct DaedalusApp {
    config_store: ConfigStore,
    backend: Backend,
    config: AppConfig,
    config_draft: AppConfig,
    library_items: Vec<LibraryItem>,
    jobs: Vec<Job>,
    active_view: View,
    selected_item_id: Option<i64>,
    selected_kind: Option<ModelKind>,
    sort_mode: LibrarySort,
    search_query: String,
    status_line: String,
}

#[cfg(feature = "desktop")]
impl DaedalusApp {
    fn bootstrap() -> Self {
        let config_store = ConfigStore::discover();
        let local_config = config_store
            .load_or_default()
            .unwrap_or_else(|_| AppConfig::default());
        let (backend, status_line) = connect_backend(&config_store, &local_config);
        let config = backend
            .current_config()
            .map(|mut backend_config| {
                backend_config.gui = local_config.gui.clone();
                backend_config
            })
            .unwrap_or(local_config);

        let mut app = Self {
            config_store,
            backend,
            config_draft: config.clone(),
            config,
            library_items: Vec::new(),
            jobs: Vec::new(),
            active_view: View::Library,
            selected_item_id: None,
            selected_kind: None,
            sort_mode: LibrarySort::Recent,
            search_query: String::new(),
            status_line,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        if let Ok(mut config) = self.backend.current_config() {
            if let Ok(local_config) = self.config_store.load_or_default() {
                config.gui = local_config.gui;
            }
            self.config = config.clone();
            self.config_draft = config;
        }

        match self.backend.list_library_items() {
            Ok(items) => self.library_items = items,
            Err(err) => self.status_line = format!("Failed to load library: {err}"),
        }

        match self.backend.list_jobs() {
            Ok(jobs) => self.jobs = jobs,
            Err(err) => self.status_line = format!("Failed to load jobs: {err}"),
        }
    }

    fn reconnect(&mut self) {
        let (backend, status_line) = connect_backend(&self.config_store, &self.config_draft);
        self.backend = backend;
        self.status_line = status_line;
        self.refresh();
    }

    fn save_settings(&mut self) {
        let validation = self.config_draft.validate();
        if let Ok(report) = &validation {
            if !report.errors.is_empty() {
                self.status_line = report.errors.join(" | ");
                return;
            }
        }

        if let Err(err) = self.config_store.save(&self.config_draft) {
            self.status_line = format!("Failed to save local config: {err}");
            return;
        }

        if let Err(err) = self.backend.update_config(&self.config_draft) {
            self.status_line = format!("Saved local config, but backend update failed: {err}");
            return;
        }

        self.status_line = format!("Saved configuration to {}", self.config_store.path().display());
        self.reconnect();
    }

    fn filtered_library_items(&self) -> Vec<LibraryItem> {
        let needle = self.search_query.trim().to_ascii_lowercase();
        let mut items = self
            .library_items
            .iter()
            .filter(|item| {
                self.selected_kind
                    .map(|kind| kind == item.primary_model_kind)
                    .unwrap_or(true)
            })
            .filter(|item| {
                if needle.is_empty() {
                    return true;
                }

                let source_match = item
                    .source
                    .as_ref()
                    .map(|source| {
                        source
                            .source_url
                            .as_deref()
                            .unwrap_or_default()
                            .to_ascii_lowercase()
                            .contains(&needle)
                            || source
                                .source_model_id
                                .as_deref()
                                .unwrap_or_default()
                                .to_ascii_lowercase()
                                .contains(&needle)
                    })
                    .unwrap_or(false);

                item.display_name.to_ascii_lowercase().contains(&needle)
                    || item.storage_path.to_ascii_lowercase().contains(&needle)
                    || source_match
            })
            .cloned()
            .collect::<Vec<_>>();

        match self.sort_mode {
            LibrarySort::Recent => items.sort_by(|left, right| right.created_at.cmp(&left.created_at)),
            LibrarySort::Name => items.sort_by(|left, right| left.display_name.cmp(&right.display_name)),
            LibrarySort::Kind => items.sort_by(|left, right| {
                left.primary_model_kind
                    .label()
                    .cmp(right.primary_model_kind.label())
                    .then(left.display_name.cmp(&right.display_name))
            }),
            LibrarySort::Path => items.sort_by(|left, right| left.storage_path.cmp(&right.storage_path)),
        }

        items
    }

    fn selected_item(&self) -> Option<&LibraryItem> {
        self.selected_item_id
            .and_then(|id| self.library_items.iter().find(|item| item.id == id))
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("Daedalus");
            ui.separator();
            ui.label(format!("Mode: {}", self.backend.label()));
            ui.separator();
            ui.label(&self.status_line);
            ui.separator();

            if ui.button("Refresh").clicked() {
                self.refresh();
            }
            if ui.button("Rescan").clicked() {
                match self.backend.rescan_library() {
                    Ok(job) => {
                        self.status_line = job.summary;
                        self.refresh();
                        self.active_view = View::Jobs;
                    }
                    Err(err) => self.status_line = format!("Rescan failed: {err}"),
                }
            }
            if ui.button("Reconnect").clicked() {
                self.reconnect();
            }
        });

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.active_view, View::Library, "Library");
            ui.selectable_value(&mut self.active_view, View::Jobs, "Jobs");
            ui.selectable_value(&mut self.active_view, View::Settings, "Settings");
        });
    }

    fn render_library(&mut self, ui: &mut egui::Ui) {
        let filtered_items = self.filtered_library_items();

        ui.horizontal(|ui| {
            ui.label("Search");
            ui.text_edit_singleline(&mut self.search_query);

            egui::ComboBox::from_label("Kind")
                .selected_text(
                    self.selected_kind
                        .map(|kind| kind.label().to_string())
                        .unwrap_or_else(|| "All".to_string()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.selected_kind, None, "All");
                    for kind in ModelKind::all() {
                        ui.selectable_value(&mut self.selected_kind, Some(*kind), kind.label());
                    }
                });

            egui::ComboBox::from_label("Sort")
                .selected_text(match self.sort_mode {
                    LibrarySort::Recent => "Recent",
                    LibrarySort::Name => "Name",
                    LibrarySort::Kind => "Kind",
                    LibrarySort::Path => "Path",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.sort_mode, LibrarySort::Recent, "Recent");
                    ui.selectable_value(&mut self.sort_mode, LibrarySort::Name, "Name");
                    ui.selectable_value(&mut self.sort_mode, LibrarySort::Kind, "Kind");
                    ui.selectable_value(&mut self.sort_mode, LibrarySort::Path, "Path");
                });
        });

        ui.label(format!(
            "{} visible / {} total",
            filtered_items.len(),
            self.library_items.len()
        ));
        ui.separator();

        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if filtered_items.is_empty() {
                        ui.label("No matching library items.");
                    }

                    for item in &filtered_items {
                        let selected = self.selected_item_id == Some(item.id);
                        if ui
                            .selectable_label(
                                selected,
                                format!("{} [{}]", item.display_name, item.primary_model_kind.label()),
                            )
                            .clicked()
                        {
                            self.selected_item_id = Some(item.id);
                        }
                        ui.small(&item.storage_path);
                        ui.separator();
                    }
                });
            });

            columns[1].vertical(|ui| {
                ui.heading("Detail");
                ui.separator();

                if let Some(item) = self.selected_item() {
                    ui.label(format!("Name: {}", item.display_name));
                    ui.label(format!("Kind: {}", item.primary_model_kind.label()));
                    ui.label(format!("Path: {}", item.storage_path));
                    ui.label(format!("Created: {}", item.created_at));
                    if let Some(verified_at) = item.last_verified_at {
                        ui.label(format!("Last verified: {}", verified_at));
                    }
                    if let Some(source) = &item.source {
                        ui.label(format!("Source: {:?}", source.source_kind));
                        if let Some(source_url) = &source.source_url {
                            ui.label(format!("Source URL: {source_url}"));
                        }
                    }
                    if let Some(notes) = &item.notes {
                        ui.label(format!("Notes: {notes}"));
                    }
                } else {
                    ui.label("Select a library item to inspect it.");
                }
            });
        });
    }

    fn render_jobs(&mut self, ui: &mut egui::Ui) {
        ui.label(format!("{} jobs", self.jobs.len()));
        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.jobs.is_empty() {
                ui.label("No jobs yet.");
            }

            for job in &self.jobs {
                ui.group(|ui| {
                    ui.label(format!("{:?} / {:?}", job.kind, job.status));
                    ui.small(&job.summary);
                    ui.small(format!("Progress: {:.0}%", job.progress * 100.0));
                    ui.small(format!("Created: {}", job.created_at));
                    if let Some(error_summary) = &job.error_summary {
                        ui.colored_label(egui::Color32::RED, error_summary);
                    }
                });
            }
        });
    }

    fn render_settings(&mut self, ui: &mut egui::Ui) {
        let validation = self
            .config_draft
            .validate()
            .unwrap_or_else(|err| ValidationReport {
                errors: vec![err.to_string()],
                warnings: Vec::new(),
            });

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Connection");
            egui::ComboBox::from_label("Default mode")
                .selected_text(match self.config_draft.gui.default_mode {
                    GuiMode::Auto => "Auto",
                    GuiMode::Embedded => "Embedded",
                    GuiMode::Remote => "Remote",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.config_draft.gui.default_mode, GuiMode::Auto, "Auto");
                    ui.selectable_value(&mut self.config_draft.gui.default_mode, GuiMode::Embedded, "Embedded");
                    ui.selectable_value(&mut self.config_draft.gui.default_mode, GuiMode::Remote, "Remote");
                });
            ui.label("Remote daemon URL");
            ui.text_edit_singleline(&mut self.config_draft.gui.remote_url);
            ui.label("Daemon host");
            ui.text_edit_singleline(&mut self.config_draft.daemon.host);
            ui.add(egui::DragValue::new(&mut self.config_draft.daemon.port).range(1..=65535));
            ui.checkbox(&mut self.config_draft.daemon.enabled, "Enable daemon mode");

            ui.separator();
            ui.heading("Storage");
            ui.label("Default storage root");
            ui.text_edit_singleline(&mut self.config_draft.library.default_storage_root);
            ui.label("Temporary download root");
            ui.text_edit_singleline(&mut self.config_draft.library.temp_download_root);
            ui.label("Preview cache root");
            ui.text_edit_singleline(&mut self.config_draft.library.preview_cache_root);
            ui.label("Database path");
            ui.text_edit_singleline(&mut self.config_draft.database.path);
            ui.checkbox(
                &mut self.config_draft.library.managed_by_kind,
                "Manage files in kind-specific paths",
            );
            ui.checkbox(
                &mut self.config_draft.library.deduplicate_by_sha256,
                "Deduplicate by sha256",
            );

            ui.separator();
            ui.heading("Model Paths");
            model_path_editor(ui, &mut self.config_draft);

            ui.separator();
            ui.heading("Civitai");
            ui.checkbox(&mut self.config_draft.sources.civitai.enabled, "Enabled");
            ui.label("API base URL");
            ui.text_edit_singleline(&mut self.config_draft.sources.civitai.api_base_url);
            ui.label("Web base URL");
            ui.text_edit_singleline(&mut self.config_draft.sources.civitai.web_base_url);
            ui.label("API token");
            ui.add(
                egui::TextEdit::singleline(&mut self.config_draft.sources.civitai.api_token).password(true),
            );
            ui.checkbox(
                &mut self.config_draft.sources.civitai.sync_preview_images,
                "Sync preview images",
            );
            ui.checkbox(
                &mut self.config_draft.sources.civitai.sync_creator_metadata,
                "Sync creator metadata",
            );

            ui.separator();
            ui.heading("Validation");
            if validation.errors.is_empty() && validation.warnings.is_empty() {
                ui.colored_label(egui::Color32::GREEN, "Configuration validates cleanly.");
            }
            for error in &validation.errors {
                ui.colored_label(egui::Color32::RED, error);
            }
            for warning in &validation.warnings {
                ui.colored_label(egui::Color32::YELLOW, warning);
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Reset Draft").clicked() {
                    self.config_draft = self.config.clone();
                }
                if ui.button("Save Settings").clicked() {
                    self.save_settings();
                }
            });
        });
    }
}

#[cfg(feature = "desktop")]
impl eframe::App for DaedalusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| self.render_toolbar(ui));

        egui::CentralPanel::default().show(ctx, |ui| match self.active_view {
            View::Library => self.render_library(ui),
            View::Jobs => self.render_jobs(ui),
            View::Settings => self.render_settings(ui),
        });
    }
}

#[cfg(feature = "desktop")]
fn connect_backend(config_store: &ConfigStore, local_config: &AppConfig) -> (Backend, String) {
    match local_config.gui.default_mode {
        GuiMode::Embedded => match DaedalusService::from_store(config_store.clone(), "embedded") {
            Ok(service) => (Backend::Embedded(service), "Embedded mode".to_string()),
            Err(err) => panic!("failed to start embedded service: {err}"),
        },
        GuiMode::Remote => {
            let client = DaedalusClient::new(local_config.gui.remote_url.clone());
            if client.health().is_ok() {
                (Backend::Remote(client), format!("Remote mode: {}", local_config.gui.remote_url))
            } else {
                let service = DaedalusService::from_store(config_store.clone(), "embedded")
                    .unwrap_or_else(|err| panic!("failed to start embedded fallback: {err}"));
                (
                    Backend::Embedded(service),
                    format!(
                        "Remote daemon unavailable at {}; fell back to embedded mode",
                        local_config.gui.remote_url
                    ),
                )
            }
        }
        GuiMode::Auto => {
            let client = DaedalusClient::new(local_config.gui.remote_url.clone());
            if client.health().is_ok() {
                (Backend::Remote(client), format!("Remote mode: {}", local_config.gui.remote_url))
            } else {
                let service = DaedalusService::from_store(config_store.clone(), "embedded")
                    .unwrap_or_else(|err| panic!("failed to start embedded service: {err}"));
                (
                    Backend::Embedded(service),
                    format!(
                        "Auto mode: remote unavailable at {}; using embedded mode",
                        local_config.gui.remote_url
                    ),
                )
            }
        }
    }
}

#[cfg(feature = "desktop")]
fn model_path_editor(ui: &mut egui::Ui, config: &mut AppConfig) {
    for kind in ModelKind::all() {
        ui.label(kind.label());
        let value = match kind {
            ModelKind::Checkpoint => &mut config.model_paths.checkpoint,
            ModelKind::Embedding => &mut config.model_paths.embedding,
            ModelKind::Hypernetwork => &mut config.model_paths.hypernetwork,
            ModelKind::AestheticGradient => &mut config.model_paths.aesthetic_gradient,
            ModelKind::LoRA => &mut config.model_paths.lora,
            ModelKind::LyCORIS => &mut config.model_paths.lycoris,
            ModelKind::DoRA => &mut config.model_paths.dora,
            ModelKind::ControlNet => &mut config.model_paths.controlnet,
            ModelKind::Upscaler => &mut config.model_paths.upscaler,
            ModelKind::Motion => &mut config.model_paths.motion,
            ModelKind::Vae => &mut config.model_paths.vae,
            ModelKind::Poses => &mut config.model_paths.poses,
            ModelKind::Wildcards => &mut config.model_paths.wildcards,
            ModelKind::Workflows => &mut config.model_paths.workflows,
            ModelKind::Detection => &mut config.model_paths.detection,
            ModelKind::Other => &mut config.model_paths.other,
        };
        ui.text_edit_singleline(value);
    }
}
