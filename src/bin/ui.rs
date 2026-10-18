use eframe::egui::{self, Color32, RichText};
#[cfg(feature = "ui-local-fixture")]
use pwds_strike_launcher::ui_fixture::{FixtureSet, check_isolated_root, open_isolated_root};
use pwds_strike_launcher::{
    APPS, InstallPlan, InstalledState, Release, TrustedManifest, app, availability,
    bundled_manifest, current_target, default_state_path,
    installer::{InstallRoot, Phase},
    plan_install,
};
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Catalog,
    Library,
}

struct LauncherUi {
    page: Page,
    manifest: Option<TrustedManifest>,
    state: Option<InstalledState>,
    verified_ids: HashSet<String>,
    error: Option<String>,
    root: Option<Arc<InstallRoot>>,
    job: Option<UiJob>,
    notice: Option<Result<String, String>>,
    confirm_uninstall: Option<String>,
    capture: Option<CaptureOnce>,
    #[cfg(feature = "ui-local-fixture")]
    fixture: Option<Arc<FixtureSet>>,
}

struct UiJob {
    app_id: String,
    phase: Arc<Mutex<Phase>>,
    cancel: Arc<AtomicBool>,
    result: Arc<Mutex<Option<Result<(), String>>>>,
}

struct CaptureOnce {
    path: PathBuf,
    outcome: Arc<Mutex<Option<Result<(), String>>>>,
    started: Instant,
    settle_frames: u8,
    requested: bool,
}

impl LauncherUi {
    fn new(
        #[cfg(feature = "ui-local-fixture")] fixture: Option<Arc<FixtureSet>>,
        #[cfg(feature = "ui-local-fixture")] fixture_root: Option<Arc<InstallRoot>>,
    ) -> Self {
        let manifest = bundled_manifest();
        let root_path = {
            #[cfg(feature = "developer-preview")]
            {
                std::env::var_os("PWDS_LAUNCHER_PREVIEW_ROOT")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| default_state_path().parent().unwrap().to_path_buf())
            }
            #[cfg(not(feature = "developer-preview"))]
            {
                default_state_path().parent().unwrap().to_path_buf()
            }
        };
        #[cfg(feature = "ui-local-fixture")]
        let root = match fixture_root {
            Some(root) => Ok(root),
            None => InstallRoot::open(&root_path).map(Arc::new),
        };
        #[cfg(not(feature = "ui-local-fixture"))]
        let root = InstallRoot::open(&root_path).map(Arc::new);
        let state = root
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|root| root.state());
        let error = manifest
            .as_ref()
            .err()
            .cloned()
            .or_else(|| state.as_ref().err().cloned())
            .or_else(|| root.as_ref().err().cloned());
        let verified_ids = match (&root, &state) {
            (Ok(root), Ok(state)) => state
                .installed
                .iter()
                .filter(|r| {
                    (r.platform.as_str(), r.arch.as_str()) == current_target()
                        && root.entrypoint(&r.app_id).is_ok()
                })
                .map(|r| r.app_id.clone())
                .collect(),
            _ => HashSet::new(),
        };
        Self {
            page: Page::Catalog,
            manifest: manifest.ok(),
            state: state.ok(),
            verified_ids,
            error,
            root: root.ok(),
            job: None,
            notice: None,
            confirm_uninstall: None,
            capture: None,
            #[cfg(feature = "ui-local-fixture")]
            fixture,
        }
    }

    fn start_job(&mut self, id: &str, action: &str) {
        if self.job.is_some() {
            return;
        }
        let (Some(root), Some(manifest), Some(state)) = (&self.root, &self.manifest, &self.state)
        else {
            return;
        };
        #[cfg(feature = "ui-local-fixture")]
        let fixture = self.fixture.clone();
        let release: Option<(Release, bool)> = if action == "install" {
            let (platform, arch) = current_target();
            #[cfg(feature = "ui-local-fixture")]
            let planned = if let Some(local) = &fixture {
                local
                    .plan(state, id)
                    .map(|plan| (plan.release.clone(), plan.is_update))
            } else {
                plan_install(manifest, state, id, platform, arch)
                    .map(|plan| (plan.release.clone(), plan.is_update))
            };
            #[cfg(not(feature = "ui-local-fixture"))]
            let planned = plan_install(manifest, state, id, platform, arch)
                .map(|plan| (plan.release.clone(), plan.is_update));
            match planned {
                Ok(plan) => Some(plan),
                Err(e) => {
                    self.notice = Some(Err(e));
                    return;
                }
            }
        } else {
            None
        };
        let root = Arc::clone(root);
        let id_owned = id.to_string();
        #[cfg(feature = "ui-local-fixture")]
        let initial_phase = if fixture.is_some() {
            Phase::Verifying
        } else {
            Phase::Downloading
        };
        #[cfg(not(feature = "ui-local-fixture"))]
        let initial_phase = Phase::Downloading;
        let phase = Arc::new(Mutex::new(initial_phase));
        let cancel = Arc::new(AtomicBool::new(false));
        let result = Arc::new(Mutex::new(None));
        let thread_phase = Arc::clone(&phase);
        let thread_cancel = Arc::clone(&cancel);
        let thread_result = Arc::clone(&result);
        let action_owned = action.to_string();
        let job_id = id_owned.clone();
        std::thread::spawn(move || {
            let outcome = match action_owned.as_str() {
                "install" => {
                    let (release, is_update) = release.expect("install has a planned release");
                    #[cfg(feature = "ui-local-fixture")]
                    if let Some(local) = fixture {
                        local
                            .install_next(&root, &mut |next| {
                                *thread_phase.lock().unwrap() = next;
                                !thread_cancel.load(Ordering::Relaxed)
                            })
                            .map(|_| ())
                    } else {
                        root.download_and_install(
                            &InstallPlan {
                                release: &release,
                                is_update,
                            },
                            &mut |next| {
                                *thread_phase.lock().unwrap() = next;
                                !thread_cancel.load(Ordering::Relaxed)
                            },
                        )
                    }
                    #[cfg(not(feature = "ui-local-fixture"))]
                    root.download_and_install(
                        &InstallPlan {
                            release: &release,
                            is_update,
                        },
                        &mut |next| {
                            *thread_phase.lock().unwrap() = next;
                            !thread_cancel.load(Ordering::Relaxed)
                        },
                    )
                }
                "launch" => root.launch(&job_id, &[]).map(|_| ()),
                "uninstall" => root.uninstall(&job_id),
                "rollback" => root.rollback(&job_id),
                _ => Err("unknown launcher action".into()),
            };
            *thread_result.lock().unwrap() = Some(outcome);
        });
        self.job = Some(UiJob {
            app_id: id_owned,
            phase,
            cancel,
            result,
        });
        self.notice = None;
    }

    fn poll_job(&mut self) {
        let Some(job) = &self.job else {
            return;
        };
        let result = job.result.lock().unwrap().take();
        if let Some(result) = result {
            let id = job.app_id.clone();
            match self.root.as_ref().map(|root| root.state()) {
                Some(Ok(state)) => {
                    self.verified_ids = state
                        .installed
                        .iter()
                        .filter(|r| {
                            (r.platform.as_str(), r.arch.as_str()) == current_target()
                                && self
                                    .root
                                    .as_ref()
                                    .is_some_and(|root| root.entrypoint(&r.app_id).is_ok())
                        })
                        .map(|r| r.app_id.clone())
                        .collect();
                    self.state = Some(state);
                    self.notice = Some(result.map(|_| format!("{id}: operation complete")));
                }
                Some(Err(error)) => {
                    self.notice = Some(Err(format!("cannot refresh library: {error}")))
                }
                None => self.notice = Some(Err("install root unavailable".into())),
            }
            self.job = None;
        }
    }

    fn capture_once(&mut self, path: PathBuf, outcome: Arc<Mutex<Option<Result<(), String>>>>) {
        self.capture = Some(CaptureOnce {
            path,
            outcome,
            started: Instant::now(),
            settle_frames: 0,
            requested: false,
        });
    }

    fn finish_capture(&mut self, ctx: &egui::Context, result: Result<(), String>) {
        if let Some(capture) = self.capture.take() {
            *capture.outcome.lock().unwrap() = Some(result);
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn capture_after_frame(&mut self, ctx: &egui::Context) {
        let Some(capture) = self.capture.as_ref() else {
            return;
        };
        if capture.started.elapsed() > Duration::from_secs(15) {
            self.finish_capture(
                ctx,
                Err("timed out waiting for the native screenshot".into()),
            );
            return;
        }
        if capture.requested {
            let image = ctx.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                    _ => None,
                })
            });
            if let Some(image) = image {
                let result = Self::write_capture_png(&capture.path, &image);
                self.finish_capture(ctx, result);
            } else {
                ctx.request_repaint();
            }
            return;
        }
        let capture = self.capture.as_mut().unwrap();
        if capture.settle_frames < 2 {
            capture.settle_frames += 1;
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            capture.requested = true;
        }
        ctx.request_repaint();
    }

    fn write_capture_png(path: &Path, screenshot: &egui::ColorImage) -> Result<(), String> {
        let [width, height] = screenshot.size;
        let capacity = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("screenshot dimensions overflow")?;
        let mut rgba = Vec::with_capacity(capacity);
        for pixel in &screenshot.pixels {
            rgba.extend_from_slice(&pixel.to_array());
        }
        let width = u32::try_from(width).map_err(|_| "screenshot width exceeds PNG limits")?;
        let height = u32::try_from(height).map_err(|_| "screenshot height exceeds PNG limits")?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|error| format!("create screenshot {}: {error}", path.display()))?;
        let mut encoder = png::Encoder::new(&mut file, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| format!("start PNG: {error}"))?;
        writer
            .write_image_data(&rgba)
            .map_err(|error| format!("write PNG: {error}"))?;
        writer
            .finish()
            .map_err(|error| format!("finish PNG: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("sync screenshot: {error}"))
    }

    fn logo_slot(ui: &mut egui::Ui) {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(76.0, 58.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, Color32::from_rgb(31, 43, 59));
        let color = Color32::from_rgb(144, 170, 190);
        painter.text(
            rect.center() + egui::vec2(0.0, -8.0),
            egui::Align2::CENTER_CENTER,
            "LOGO",
            egui::FontId::proportional(13.0),
            color,
        );
        painter.text(
            rect.center() + egui::vec2(0.0, 9.0),
            egui::Align2::CENTER_CENTER,
            "pending",
            egui::FontId::proportional(10.0),
            color,
        );
    }

    fn catalog(&mut self, ui: &mut egui::Ui) {
        let (Some(manifest), Some(state)) = (&self.manifest, &self.state) else {
            return;
        };
        let (platform, arch) = current_target();
        ui.label(
            RichText::new("Choose the tools you want")
                .size(23.0)
                .strong(),
        );
        ui.label("Choose a verified Strike release to install. Apps without a packaged release are coming soon.");
        ui.add_space(14.0);
        let mut selected = None;
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
                                #[cfg(feature = "ui-local-fixture")]
                                let status = if self.fixture.is_some() && item.id == "gridstrike" {
                                    "Explicitly pinned local development fixture"
                                } else {
                                    availability(manifest, state, item.id, platform, arch)
                                };
                                #[cfg(not(feature = "ui-local-fixture"))]
                                let status = availability(manifest, state, item.id, platform, arch);
                                ui.label(
                                    RichText::new(status).color(Color32::from_rgb(202, 177, 114)),
                                );
                            });
                        });
                        ui.add_space(5.0);
                        #[cfg(feature = "ui-local-fixture")]
                        let planned = if let Some(local) = &self.fixture {
                            local.plan(state, item.id).is_ok()
                        } else {
                            plan_install(manifest, state, item.id, platform, arch).is_ok()
                        };
                        #[cfg(not(feature = "ui-local-fixture"))]
                        let planned =
                            plan_install(manifest, state, item.id, platform, arch).is_ok();
                        let available = planned
                            && (state.receipt(item.id).is_none()
                                || self.verified_ids.contains(item.id));
                        let label = if state.receipt(item.id).is_some() {
                            "Update"
                        } else {
                            "Install"
                        };
                        if ui
                            .add_enabled(available && self.job.is_none(), egui::Button::new(label))
                            .clicked()
                        {
                            selected = Some(item.id);
                        }
                    });
                ui.add_space(8.0);
            }
        });
        if let Some(id) = selected {
            self.start_job(id, "install");
        }
    }

    fn library(&mut self, ui: &mut egui::Ui) {
        let Some(state) = &self.state else {
            return;
        };
        ui.label(RichText::new("Your library").size(23.0).strong());
        ui.label("Launcher-managed installations appear here. Uninstall preserves your documents and settings.");
        ui.add_space(12.0);
        let mut action = None;
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
                            let idle =
                                self.job.is_none() && self.verified_ids.contains(&receipt.app_id);
                            if self.job.is_none() && !idle {
                                let message = if (receipt.platform.as_str(), receipt.arch.as_str())
                                    != current_target()
                                {
                                    "Installed for another platform"
                                } else {
                                    "Installation needs verification or repair"
                                };
                                ui.colored_label(Color32::LIGHT_RED, message);
                            }
                            if ui.add_enabled(idle, egui::Button::new("Launch")).clicked() {
                                action = Some((receipt.app_id.clone(), "launch"));
                            }
                            let (platform, arch) = current_target();
                            #[cfg(feature = "ui-local-fixture")]
                            let update = if let Some(local) = &self.fixture {
                                local.plan(state, &receipt.app_id).is_ok()
                            } else {
                                self.manifest.as_ref().is_some_and(|manifest| {
                                    plan_install(manifest, state, &receipt.app_id, platform, arch)
                                        .is_ok()
                                })
                            };
                            #[cfg(not(feature = "ui-local-fixture"))]
                            let update = self.manifest.as_ref().is_some_and(|manifest| {
                                plan_install(manifest, state, &receipt.app_id, platform, arch)
                                    .is_ok()
                            });
                            if ui
                                .add_enabled(idle && update, egui::Button::new("Update"))
                                .clicked()
                            {
                                action = Some((receipt.app_id.clone(), "install"));
                            }
                            let rollback = self
                                .root
                                .as_ref()
                                .is_some_and(|root| root.can_rollback(&receipt.app_id));
                            if ui
                                .add_enabled(idle && rollback, egui::Button::new("Rollback"))
                                .clicked()
                            {
                                action = Some((receipt.app_id.clone(), "rollback"));
                            }
                            if ui
                                .add_enabled(idle, egui::Button::new("Uninstall"))
                                .clicked()
                            {
                                action = Some((receipt.app_id.clone(), "confirm-uninstall"));
                            }
                        });
                    });
                });
            });
            ui.add_space(8.0);
        }
        if let Some((id, operation)) = action {
            if operation == "confirm-uninstall" {
                self.confirm_uninstall = Some(id);
            } else {
                self.start_job(&id, operation);
            }
        }
    }
}

impl eframe::App for LauncherUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job();
        if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
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
            #[cfg(feature = "developer-preview")]
            if std::env::var_os("PWDS_LAUNCHER_PREVIEW_ROOT").is_some()
                || cfg!(feature = "ui-local-fixture") && {
                    #[cfg(feature = "ui-local-fixture")]
                    {
                        self.fixture.is_some()
                    }
                    #[cfg(not(feature = "ui-local-fixture"))]
                    {
                        false
                    }
                }
            {
                ui.colored_label(
                    Color32::YELLOW,
                    "DEVELOPMENT PREVIEW - locally built package, not a public release",
                );
            }
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.page, Page::Catalog, "Catalog");
                ui.selectable_value(&mut self.page, Page::Library, "Library");
            });
            ui.add_space(6.0);
        });
        egui::TopBottomPanel::bottom("operations").show(ctx, |ui| {
            if let Some(job) = &self.job {
                let phase = *job.phase.lock().unwrap();
                ui.horizontal(|ui| {
                    ui.spinner();
                    match phase {
                        Phase::DownloadBytes(done, total) => {
                            ui.label(format!("{}: downloading {done}/{total} bytes", job.app_id));
                        }
                        Phase::ExtractedEntries(done, total) => {
                            ui.label(format!("{}: extracting {done}/{total} entries", job.app_id));
                        }
                        other => {
                            ui.label(format!("{}: {other:?}", job.app_id));
                        }
                    }
                    if ui
                        .add_enabled(phase != Phase::Activating, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                });
            }
            if let Some(notice) = &self.notice {
                match notice {
                    Ok(message) => {
                        ui.colored_label(Color32::LIGHT_GREEN, message);
                    }
                    Err(message) => {
                        ui.colored_label(Color32::LIGHT_RED, message);
                    }
                }
            }
        });
        if let Some(id) = self.confirm_uninstall.clone() {
            let mut confirm = false;
            let mut close = false;
            egui::Window::new("Uninstall Strike app")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(format!("Remove the launcher-owned version of {id}?"));
                    ui.label("Your documents and settings will be preserved.");
                    ui.horizontal(|ui| {
                        if ui.button("Uninstall").clicked() {
                            confirm = true;
                            close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            close = true;
                        }
                    });
                });
            if close {
                self.confirm_uninstall = None;
            }
            if confirm {
                self.start_job(&id, "uninstall");
            }
        }
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
        self.capture_after_frame(ctx);
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PWDS Strike Launcher: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    #[cfg(feature = "ui-local-fixture")]
    let fixture_launch: Option<(Arc<FixtureSet>, Arc<InstallRoot>)> = match args.as_slice() {
        [flag, spec, root] if flag == "--check-local-fixture" => {
            let fixture = FixtureSet::load(Path::new(spec))?;
            check_isolated_root(Path::new(root))?;
            println!(
                "GridStrike {} local fixture verified; isolated root available",
                fixture.first_version()
            );
            return Ok(());
        }
        [flag, spec, root] if flag == "--local-fixture-install" => {
            let fixture = FixtureSet::load(Path::new(spec))?;
            let root = open_isolated_root(Path::new(root))?;
            let version = fixture.install_next(&root, &mut |_| true)?;
            println!("GridStrike {version} installed from pinned local UI fixture");
            return Ok(());
        }
        [flag, spec, root] if flag == "--local-fixture" => {
            let fixture = Arc::new(FixtureSet::load(Path::new(spec))?);
            let root = Arc::new(open_isolated_root(Path::new(root))?);
            Some((fixture, root))
        }
        _ => None,
    };
    let capture = match args.as_slice() {
        [] => None,
        [flag, page, output] if flag == "--capture" => {
            let page = match page.to_str() {
                Some("catalog") => Page::Catalog,
                Some("library") => Page::Library,
                _ => return Err("capture page must be catalog or library".into()),
            };
            Some((page, PathBuf::from(output)))
        }
        #[cfg(feature = "ui-local-fixture")]
        [flag, _, _] if flag == "--local-fixture" => None,
        _ => {
            return Err(
                "usage: pwds-strike-launcher-ui [--capture catalog|library NEW.png]".into(),
            );
        }
    };
    let outcome = Arc::new(Mutex::new(None));
    let captured = capture.is_some();
    let result = Arc::clone(&outcome);
    let options = eframe::NativeOptions {
        renderer: if captured && !cfg!(target_os = "windows") {
            eframe::Renderer::Glow
        } else {
            eframe::Renderer::Wgpu
        },
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 760.0])
            .with_min_inner_size([780.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native(
        "PWDS Strike Launcher",
        options,
        Box::new(move |_| {
            #[cfg(feature = "ui-local-fixture")]
            let mut app = match fixture_launch {
                Some((fixture, root)) => LauncherUi::new(Some(fixture), Some(root)),
                None => LauncherUi::new(None, None),
            };
            #[cfg(not(feature = "ui-local-fixture"))]
            let mut app = LauncherUi::new();
            if let Some((page, path)) = capture {
                app.page = page;
                app.capture_once(path, result);
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| error.to_string())?;
    if captured {
        outcome
            .lock()
            .unwrap()
            .take()
            .ok_or("native window closed before capture finished")??;
    }
    Ok(())
}
