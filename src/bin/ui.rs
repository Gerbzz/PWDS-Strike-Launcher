use eframe::egui::{self, Color32, RichText};
use pwds_strike_launcher::{
    APPS, InstalledState, TrustedManifest, app, availability, bundled_manifest, current_target,
    default_state_path,
};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Catalog,
    Library,
}

struct LauncherUi {
    page: Page,
    manifest: Option<TrustedManifest>,
    state: Option<InstalledState>,
    error: Option<String>,
}

impl LauncherUi {
    fn new() -> Self {
        let manifest = bundled_manifest();
        let state = InstalledState::load(&default_state_path());
        let error = manifest
            .as_ref()
            .err()
            .cloned()
            .or_else(|| state.as_ref().err().cloned());
        Self {
            page: Page::Catalog,
            manifest: manifest.ok(),
            state: state.ok(),
            error,
        }
    }

    fn logo_slot(ui: &mut egui::Ui) {
        egui::Frame::none()
            .fill(Color32::from_rgb(31, 43, 59))
            .show(ui, |ui| {
                ui.set_min_size(egui::vec2(76.0, 58.0));
                ui.vertical_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("LOGO")
                            .strong()
                            .color(Color32::from_rgb(144, 170, 190)),
                    );
                    ui.label(
                        RichText::new("pending")
                            .small()
                            .color(Color32::from_rgb(144, 170, 190)),
                    );
                });
            });
    }

    fn catalog(&self, ui: &mut egui::Ui) {
        let (Some(manifest), Some(state)) = (&self.manifest, &self.state) else {
            return;
        };
        let (platform, arch) = current_target();
        ui.label(
            RichText::new("Choose the tools you want")
                .size(23.0)
                .strong(),
        );
        ui.label("One catalog for the Strike family. Downloads open when verified releases and the installer are ready.");
        ui.add_space(14.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            for item in APPS {
                egui::Frame::group(ui.style())
                    .fill(Color32::from_rgb(24, 32, 43))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            Self::logo_slot(ui);
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.label(RichText::new(item.name).size(20.0).strong());
                                ui.label(
                                    RichText::new(item.category)
                                        .color(Color32::from_rgb(109, 196, 208)),
                                );
                                ui.label(item.description);
                                ui.label(
                                    RichText::new(availability(
                                        manifest, state, item.id, platform, arch,
                                    ))
                                    .color(Color32::from_rgb(202, 177, 114)),
                                );
                            });
                        });
                        ui.add_space(5.0);
                        ui.add_enabled(false, egui::Button::new("Install / update unavailable"));
                    });
                ui.add_space(8.0);
            }
        });
    }

    fn library(&self, ui: &mut egui::Ui) {
        let Some(state) = &self.state else {
            return;
        };
        ui.label(RichText::new("Your library").size(23.0).strong());
        ui.label("Launcher-managed installations appear here. App documents and settings stay yours when an uninstall flow is added.");
        ui.add_space(12.0);
        if state.installed.is_empty() {
            ui.label("No launcher-managed apps installed yet.");
        }
        for receipt in &state.installed {
            let Some(item) = app(&receipt.app_id) else {
                continue;
            };
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    Self::logo_slot(ui);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(item.name).size(20.0).strong());
                        ui.label(format!(
                            "Version {} · {} / {}",
                            receipt.version, receipt.platform, receipt.arch
                        ));
                        ui.horizontal(|ui| {
                            ui.add_enabled(false, egui::Button::new("Launch"));
                            ui.add_enabled(false, egui::Button::new("Update"));
                            ui.add_enabled(false, egui::Button::new("Uninstall"));
                        });
                    });
                });
            });
            ui.add_space(8.0);
        }
    }
}

impl eframe::App for LauncherUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_visuals(egui::Visuals::dark());
        egui::TopBottomPanel::top("brand").show(ctx, |ui| {
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("PWDS")
                        .size(26.0)
                        .strong()
                        .color(Color32::from_rgb(112, 213, 226)),
                );
                ui.separator();
                ui.label(RichText::new("Strike Launcher").size(24.0).strong());
            });
            ui.label(RichText::new("The Strike family, in one place").color(Color32::LIGHT_GRAY));
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.page, Page::Catalog, "Catalog");
                ui.selectable_value(&mut self.page, Page::Library, "Library");
            });
            ui.add_space(6.0);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(error) = &self.error {
                ui.colored_label(
                    Color32::LIGHT_RED,
                    format!("Catalog or library unavailable: {error}"),
                );
                return;
            }
            match self.page {
                Page::Catalog => self.catalog(ui),
                Page::Library => self.library(ui),
            }
        });
    }
}

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 760.0])
            .with_min_inner_size([780.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PWDS Strike Launcher",
        options,
        Box::new(|_| Ok(Box::new(LauncherUi::new()))),
    )
}
