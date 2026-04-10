#[cfg(feature = "desktop")]
use daedalus_client::DaedalusClient;
#[cfg(feature = "desktop")]
use daedalus_config::{AppConfig, ConfigStore, GuiMode};
#[cfg(feature = "desktop")]
use daedalus_domain::LibraryItem;
#[cfg(feature = "desktop")]
use daedalus_service::DaedalusService;

#[cfg(feature = "desktop")]
fn main() -> Result<(), eframe::Error> {
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
struct DaedalusApp {
    backend: Backend,
    config: AppConfig,
    library_items: Vec<LibraryItem>,
    status_line: String,
}

#[cfg(feature = "desktop")]
enum Backend {
    Embedded(DaedalusService),
    Remote(DaedalusClient),
}

#[cfg(feature = "desktop")]
impl DaedalusApp {
    fn bootstrap() -> Self {
        let store = ConfigStore::discover();
        let config = store.load_or_default().unwrap_or_else(|_| AppConfig::default());

        let backend = match config.gui.default_mode {
            GuiMode::Auto | GuiMode::Remote => {
                let client = DaedalusClient::new(config.gui.remote_url.clone());
                if client.health().is_ok() {
                    Backend::Remote(client)
                } else {
                    Backend::Embedded(
                        DaedalusService::from_store(store, "embedded").unwrap_or_else(|err| {
                            panic!("failed to start embedded service: {err}")
                        }),
                    )
                }
            }
            GuiMode::Embedded => Backend::Embedded(
                DaedalusService::from_store(store, "embedded")
                    .unwrap_or_else(|err| panic!("failed to start embedded service: {err}")),
            ),
        };

        let mut app = Self {
            backend,
            config,
            library_items: Vec::new(),
            status_line: "Starting".to_string(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match &self.backend {
            Backend::Embedded(service) => {
                self.library_items = service.list_library_items().unwrap_or_default();
                self.status_line = "Embedded mode".to_string();
            }
            Backend::Remote(client) => {
                self.library_items = client.list_library_items().unwrap_or_default();
                self.status_line = format!("Remote mode: {}", self.config.gui.remote_url);
            }
        }
    }
}

#[cfg(feature = "desktop")]
impl eframe::App for DaedalusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Daedalus");
                ui.separator();
                ui.label(&self.status_line);
                if ui.button("Refresh").clicked() {
                    self.refresh();
                }
            });
        });

        egui::SidePanel::left("settings").show(ctx, |ui| {
            ui.heading("Settings");
            ui.label(format!(
                "Storage root: {}",
                self.config.library.default_storage_root
            ));
            ui.label(format!("Database: {}", self.config.database.path));
            ui.label(format!("Daemon URL: {}", self.config.gui.remote_url));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Library");
            ui.label(format!("{} items", self.library_items.len()));
            ui.separator();

            if self.library_items.is_empty() {
                ui.label("No library items yet. Use the CLI or API import/rescan paths first.");
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                for item in &self.library_items {
                    ui.group(|ui| {
                        ui.label(&item.display_name);
                        ui.small(format!(
                            "{} | {}",
                            item.primary_model_kind.label(),
                            item.storage_path
                        ));
                    });
                }
            });
        });
    }
}
