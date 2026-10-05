mod i18n;
mod themes;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;
use i18n::{Msg, tr};
use themes::{apply_theme, THEMES};
use sidecar_config::{
    apply_preset, default_config_path, load_config, matching_preset, reset_rendering_settings,
    save_config, sidecar_dir, Config, NeuralStrength, PRESETS,
};
use sidecar_core::{list_wow_windows, smart_scan_installs, SmartScanOptions, WowInstall};
use sidecar_install::{install_component, setup_page_data, SetupPageData};
use sidecar_probes::{run_all_probes, ProbeResult, ProbeState};
use sidecar_runtime::{is_running, read, send, start_daemon, SidecarCommand, SidecarStatus};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([900.0, 620.0]),
        ..Default::default()
    };
    eframe::run_native(
        "WoW Sidecar Manager",
        options,
        Box::new(|cc| {
            let app = ManagerApp::new(cc);
            cc.egui_ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                tr(&app.config.language, Msg::AppTitle).to_string(),
            ));
            Ok(Box::new(app))
        }),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Status,
    Setup,
    Checks,
    Tuning,
    Log,
}

impl Section {
    fn label_i18n(self, lang: &str) -> &'static str {
        match self {
            Section::Status => tr(lang, Msg::SectionStatus),
            Section::Setup => tr(lang, Msg::SectionSetup),
            Section::Checks => tr(lang, Msg::SectionChecks),
            Section::Tuning => tr(lang, Msg::SectionTuning),
            Section::Log => tr(lang, Msg::SectionLog),
        }
    }

    const ALL: [Section; 5] = [
        Section::Status,
        Section::Setup,
        Section::Checks,
        Section::Tuning,
        Section::Log,
    ];
}

struct LiveState {
    wow_running: bool,
    wow_borderless: bool,
    wow_width: u32,
    wow_height: u32,
    overlay_running: bool,
    status: Option<SidecarStatus>,
}

fn poll_live_state() -> LiveState {
    let windows = list_wow_windows();
    let wow = windows.first();
    let mut live = LiveState {
        wow_running: wow.is_some(),
        wow_borderless: wow.map(|w| !w.fullscreen).unwrap_or(false),
        wow_width: wow.map(|w| w.width).unwrap_or(0),
        wow_height: wow.map(|w| w.height).unwrap_or(0),
        overlay_running: is_running(),
        status: None,
    };
    if live.overlay_running {
        live.status = read();
    }
    live
}

fn daemon_exe_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("wowsidecar-daemon")))
        .unwrap_or_else(|| PathBuf::from("wowsidecar-daemon"))
}

struct ManagerApp {
    section: Section,
    config: Config,
    config_path: PathBuf,
    saved_snapshot: Config,
    dirty: bool,
    log_lines: Vec<String>,
    installs: Vec<WowInstall>,
    probes: Vec<ProbeResult>,
    scan_busy: bool,
    scan_rx: Option<Receiver<Vec<WowInstall>>>,
    status_message: String,
    live: LiveState,
    setup: SetupPageData,
    last_live_poll: Instant,
    setup_message: String,
    setup_message_is_error: bool,
}

impl ManagerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = default_config_path();
        let (config, warnings) = load_config(&config_path);
        apply_theme(&cc.egui_ctx, &config.theme);
        let mut app = Self {
            section: Section::Status,
            saved_snapshot: config.clone(),
            config,
            config_path,
            dirty: false,
            log_lines: Vec::new(),
            installs: Vec::new(),
            probes: Vec::new(),
            scan_busy: false,
            scan_rx: None,
            status_message: String::new(),
            live: poll_live_state(),
            setup: setup_page_data(&sidecar_dir()),
            last_live_poll: Instant::now(),
            setup_message: String::new(),
            setup_message_is_error: false,
        };
        app.log(format!("config: {}", app.config_path.display()));
        for w in warnings {
            app.log(format!("config warning: {w}"));
        }
        app.refresh_probes();
        app
    }

    fn log(&mut self, line: impl Into<String>) {
        self.log_lines.push(line.into());
        if self.log_lines.len() > 500 {
            let drop = self.log_lines.len() - 400;
            self.log_lines.drain(0..drop);
        }
    }

    fn wow_dir_path(&self) -> PathBuf {
        PathBuf::from(&self.config.wow_dir)
    }

    fn refresh_probes(&mut self) {
        self.probes = run_all_probes(&sidecar_dir(), &self.wow_dir_path());
    }

    fn refresh_setup(&mut self) {
        self.setup = setup_page_data(&sidecar_dir());
    }

    fn poll_live_if_due(&mut self, ctx: &egui::Context) {
        ctx.request_repaint_after(Duration::from_secs(1));
        if self.last_live_poll.elapsed() >= Duration::from_secs(1) {
            self.live = poll_live_state();
            self.last_live_poll = Instant::now();
        }
    }

    fn probes_blocked(&self) -> bool {
        self.probes.iter().any(|p| p.state == ProbeState::Fail)
    }

    fn can_start_overlay(&self) -> bool {
        !self.probes_blocked()
            && self.live.wow_running
            && self.setup.missing_required_components == 0
    }

    fn start_overlay(&mut self) {
        if self.dirty {
            self.save_settings();
            if self.dirty {
                return;
            }
        }
        let daemon = daemon_exe_path();
        if !daemon.is_file() {
            self.status_message = format!(
                "Missing {} — build with: cargo build -p sidecar-runtime",
                daemon.display()
            );
            self.log(self.status_message.clone());
            return;
        }
        match start_daemon(&daemon) {
            Ok(()) => {
                self.status_message = "Overlay started.".into();
                self.log("overlay daemon start requested");
                self.live = poll_live_state();
            }
            Err(err) => {
                self.status_message = err.to_string();
                self.log(format!("overlay start failed: {err}"));
            }
        }
    }

    fn stop_overlay(&mut self) {
        if send(SidecarCommand::Stop) {
            self.status_message = "Stop sent to overlay.".into();
            self.log("overlay stop sent");
        } else {
            self.status_message = "Overlay did not answer (may already be stopping).".into();
            self.log(self.status_message.clone());
        }
        self.live = poll_live_state();
    }

    fn draw_primary_overlay_button(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.live.overlay_running {
                if ui
                    .button(egui::RichText::new("Stop overlay").strong())
                    .clicked()
                {
                    self.stop_overlay();
                }
            } else {
                let can_start = self.can_start_overlay();
                ui.add_enabled_ui(can_start, |ui| {
                    if ui
                        .button(egui::RichText::new("Start overlay").strong())
                        .clicked()
                    {
                        self.start_overlay();
                    }
                });
                if !can_start {
                    ui.label(
                        egui::RichText::new(
                            "Need WoW running, passing checks, and required sidecar files.",
                        )
                        .small()
                        .weak(),
                    );
                }
            }
        });
    }

    fn mark_dirty(&mut self, dirty: bool) {
        self.dirty = dirty;
    }

    fn poll_scan(&mut self) {
        let Some(rx) = self.scan_rx.as_ref() else {
            return;
        };
        if let Ok(installs) = rx.try_recv() {
            self.installs = installs;
            self.scan_busy = false;
            self.scan_rx = None;
            self.log(format!(
                "smart scan: {} install(s) found",
                self.installs.len()
            ));
            if self.config.wow_dir.is_empty() {
                if let Some(best) = self.installs.first() {
                    self.config.wow_dir = best.game_dir.display().to_string();
                    self.mark_dirty(true);
                    self.log(format!("set WoW folder to {}", self.config.wow_dir));
                }
            }
            self.refresh_probes();
        }
    }

    fn start_smart_scan(&mut self) {
        if self.scan_busy {
            return;
        }
        self.scan_busy = true;
        self.log("smart scan started");
        let (tx, rx) = mpsc::channel();
        self.scan_rx = Some(rx);
        thread::spawn(move || {
            let installs = smart_scan_installs(&SmartScanOptions::default());
            let _ = tx.send(installs);
        });
    }

    fn save_settings(&mut self) {
        match save_config(&self.config_path, &self.config) {
            Ok(()) => {
                self.saved_snapshot = self.config.clone();
                self.mark_dirty(false);
                self.status_message = tr(&self.config.language, Msg::SettingsSaved).to_string();
                self.log("settings saved");
            }
            Err(err) => {
                self.status_message = err.clone();
                self.log(format!("save failed: {err}"));
            }
        }
    }

    fn draw_nav(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            let lang = &self.config.language;
            ui.heading(tr(lang, Msg::NavHeading));
            ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            ui.add_space(8.0);
            for sec in Section::ALL {
                if sec == Section::Log && !self.config.advanced_mode {
                    continue;
                }
                let selected = self.section == sec;
                let mut label = sec.label_i18n(lang).to_string();
                if sec == Section::Tuning && self.dirty {
                    label.push('*');
                }
                if (sec == Section::Setup && self.setup.missing_required_components > 0)
                    || (sec == Section::Checks && self.probes_blocked())
                {
                    label.push('!');
                }
                if ui.selectable_label(selected, label).clicked() {
                    self.section = sec;
                    if sec == Section::Setup {
                        self.refresh_setup();
                    }
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(format!(
                    "{}: {}",
                    tr(lang, Msg::ThemeLabel),
                    themes::normalize_theme(&self.config.theme)
                ));
                ui.label(format!(
                    "{}: {}",
                    tr(lang, Msg::LanguageLabel),
                    self.config.language
                ));
                ui.separator();
                let wow_dot = if self.live.wow_running && self.live.wow_borderless {
                    egui::Color32::from_rgb(80, 180, 120)
                } else if self.live.wow_running {
                    egui::Color32::from_rgb(220, 180, 60)
                } else {
                    egui::Color32::GRAY
                };
                ui.horizontal(|ui| {
                    ui.colored_label(wow_dot, "●");
                    ui.label(if self.live.wow_running {
                        if self.live.wow_borderless {
                            "WoW: borderless/windowed"
                        } else {
                            "WoW: fullscreen"
                        }
                    } else {
                        "WoW: not running"
                    });
                });
                if self.live.wow_running {
                    ui.label(format!(
                        "   {}x{}",
                        self.live.wow_width, self.live.wow_height
                    ));
                }
                let overlay_dot = if self.live.overlay_running {
                    egui::Color32::from_rgb(80, 180, 120)
                } else {
                    egui::Color32::GRAY
                };
                ui.horizontal(|ui| {
                    ui.colored_label(overlay_dot, "●");
                    ui.label(if self.live.overlay_running {
                        "Overlay: running"
                    } else {
                        "Overlay: stopped"
                    });
                });
            });
        });
    }

    fn draw_status(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr(&self.config.language, Msg::SectionStatus));
        self.draw_primary_overlay_button(ui);
        ui.add_space(8.0);
        if !self.status_message.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_GREEN, &self.status_message);
        }
        ui.separator();

        ui.strong("Live");
        ui.add_space(4.0);
        if let Some(s) = self.live.status.clone() {
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label("Overlay FPS");
                    ui.strong(format!("{:.0}", s.fps));
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.label("Capture FPS");
                    ui.strong(format!("{:.0}", s.capture_fps));
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.label("Latency p50");
                    ui.strong(format!("{:.2} ms", s.p50_ms));
                });
                ui.add_space(24.0);
                ui.vertical(|ui| {
                    ui.label("Latency p99");
                    ui.strong(format!("{:.2} ms", s.p99_ms));
                });
            });
            if s.width > 0 && s.height > 0 {
                ui.label(format!("Pipeline: {}x{}", s.width, s.height));
            }
            ui.label(format!(
                "Variant: {} | pass: {}",
                s.runtime_variant, s.pass_name
            ));
            if !s.last_error.is_empty() {
                ui.colored_label(egui::Color32::LIGHT_RED, &s.last_error);
            }

            ui.add_space(8.0);
            let visible = s.overlay_visible != 0;
            let hud_up = s.hud_visible != 0;
            let mut toggle_overlay = false;
            let mut toggle_hud = false;
            ui.horizontal(|ui| {
                if ui
                    .button(if visible {
                        "Hide overlay (A/B compare)"
                    } else {
                        "Show overlay"
                    })
                    .clicked()
                {
                    toggle_overlay = true;
                }
                if ui
                    .button(if hud_up { "Hide HUD" } else { "Show HUD" })
                    .clicked()
                {
                    toggle_hud = true;
                }
            });
            if toggle_overlay {
                let cmd = if visible {
                    SidecarCommand::HideOverlay
                } else {
                    SidecarCommand::ShowOverlay
                };
                let _ = send(cmd);
                self.live = poll_live_state();
            }
            if toggle_hud {
                let cmd = if hud_up {
                    SidecarCommand::HideHud
                } else {
                    SidecarCommand::ShowHud
                };
                let _ = send(cmd);
                self.live = poll_live_state();
            }
        } else if self.live.overlay_running {
            ui.label("Overlay is running but status is not available yet.");
        } else {
            ui.label("Start the overlay to see live FPS and latency.");
        }

        ui.add_space(12.0);
        ui.separator();
        ui.label(format!("Config: {}", self.config_path.display()));
        ui.label(format!(
            "WoW folder: {}",
            if self.config.wow_dir.is_empty() {
                "(not set)".into()
            } else {
                self.config.wow_dir.clone()
            }
        ));
        ui.label(format!("WoW windows: {}", list_wow_windows().len()));
        ui.label(format!(
            "Installs on disk (last scan): {}",
            self.installs.len()
        ));

        if self.probes_blocked() {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Some checks failed. Open Checks before expecting capture to work.",
            );
        } else if self.setup.missing_required_components > 0 {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Required sidecar files missing. Open Setup.",
            );
        } else {
            ui.colored_label(egui::Color32::LIGHT_GREEN, "No blocking check failures.");
        }
    }

    fn draw_setup(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr(&self.config.language, Msg::SectionSetup));
        ui.label(
            "Required files sit next to the sidecar binary. Nothing here downloads \
             from the network — fetch artifacts yourself, then install them here.",
        );
        ui.add_space(8.0);

        ui.strong("Required files");
        let component_rows: Vec<_> = self.setup.components.clone();
        for row in component_rows {
            let c = row.component.clone();
            let present = row.present;
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    let color = if present {
                        egui::Color32::from_rgb(80, 180, 120)
                    } else if c.required {
                        egui::Color32::from_rgb(220, 90, 90)
                    } else {
                        egui::Color32::from_rgb(220, 180, 60)
                    };
                    ui.colored_label(
                        color,
                        if present {
                            "INSTALLED"
                        } else if c.required {
                            "MISSING"
                        } else {
                            "OPTIONAL"
                        },
                    );
                    ui.strong(&c.title);
                });
                ui.label(&c.purpose);
                ui.label(format!(
                    "Wanted as {} | Source: {}",
                    c.installed_as, c.source
                ));
                if ui
                    .button(if present {
                        "Replace…"
                    } else {
                        "Choose file…"
                    })
                    .clicked()
                {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        if !sidecar_install::file_matches_component(&c, &name) {
                            self.setup_message = format!(
                                "{name} is not what this slot wants ({}).",
                                c.installed_as
                            );
                            self.setup_message_is_error = true;
                        } else {
                            let result =
                                install_component(&c, &path, &sidecar_dir());
                            self.setup_message = result.message.clone();
                            self.setup_message_is_error = !result.ok;
                            if result.ok {
                                self.log(self.setup_message.clone());
                                self.refresh_setup();
                                self.refresh_probes();
                            }
                        }
                    }
                }
            });
            ui.add_space(4.0);
        }

        if !self.setup_message.is_empty() {
            let color = if self.setup_message_is_error {
                egui::Color32::LIGHT_RED
            } else {
                egui::Color32::LIGHT_GREEN
            };
            ui.colored_label(color, &self.setup_message);
        }

        ui.add_space(12.0);
        ui.strong("Runtime dependencies");
        for dep_row in &self.setup.runtime_deps {
            let d = &dep_row.dep;
            ui.horizontal(|ui| {
                let color = if dep_row.present {
                    egui::Color32::from_rgb(80, 180, 120)
                } else if d.required {
                    egui::Color32::from_rgb(220, 90, 90)
                } else {
                    egui::Color32::from_rgb(220, 180, 60)
                };
                ui.colored_label(
                    color,
                    if dep_row.present { "OK" } else { "MISSING" },
                );
                ui.label(&d.title);
            });
            ui.label(&d.purpose);
            ui.label(format!(
                "Packages: {} | probe: {}",
                d.packages_hint, d.probe_binary
            ));
            if let Some(ref path) = dep_row.probe_path {
                ui.label(format!("Found: {path}"));
            }
        }

        ui.add_space(16.0);
        ui.separator();
        ui.strong("WoW install folder");
        ui.horizontal(|ui| {
            let label = if self.scan_busy {
                "Scanning…"
            } else {
                "Smart scan for WoW folders"
            };
            if ui
                .add_enabled(!self.scan_busy, egui::Button::new(label))
                .clicked()
            {
                self.start_smart_scan();
            }
        });

        if !self.installs.is_empty() {
            ui.add_space(8.0);
            ui.label("Found installs (best first):");
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    let installs = self.installs.clone();
                    for (i, install) in installs.iter().enumerate() {
                        let tag = if i == 0 { " (best)" } else { "" };
                        ui.horizontal(|ui| {
                            ui.label(format!(
                                "{}{} — {}",
                                install.game_dir.display(),
                                tag,
                                install.client_exe
                            ));
                            if ui.button("Use").clicked() {
                                self.config.wow_dir = install.game_dir.display().to_string();
                                self.mark_dirty(true);
                                self.refresh_probes();
                            }
                        });
                    }
                });
        }

        ui.add_space(8.0);
        ui.label("WoW game folder (injector scan only — never install the sidecar here):");
        let mut wow_dir = self.config.wow_dir.clone();
        if ui.text_edit_singleline(&mut wow_dir).changed() {
            self.config.wow_dir = wow_dir;
            self.mark_dirty(true);
        }
        if ui.button("Re-run checks for this folder").clicked() {
            self.refresh_probes();
        }
    }

    fn probe_color(state: ProbeState) -> egui::Color32 {
        match state {
            ProbeState::Ok => egui::Color32::from_rgb(80, 180, 120),
            ProbeState::Warn => egui::Color32::from_rgb(220, 180, 60),
            ProbeState::Fail => egui::Color32::from_rgb(220, 90, 90),
        }
    }

    fn probe_state_label(state: ProbeState) -> &'static str {
        match state {
            ProbeState::Ok => "OK",
            ProbeState::Warn => "WARN",
            ProbeState::Fail => "FAIL",
        }
    }

    fn draw_checks(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr(&self.config.language, Msg::SectionChecks));
        ui.horizontal(|ui| {
            if ui.button("Refresh probes").clicked() {
                self.refresh_probes();
                self.log("probes refreshed");
            }
        });
        ui.add_space(6.0);

        egui::Grid::new("probes_grid")
            .num_columns(3)
            .spacing([12.0, 6.0])
            .striped(true)
            .show(ui, |ui| {
                ui.strong("State");
                ui.strong("Check");
                ui.strong("Detail");
                ui.end_row();
                for probe in &self.probes {
                    ui.colored_label(
                        Self::probe_color(probe.state),
                        Self::probe_state_label(probe.state),
                    );
                    ui.label(&probe.title);
                    ui.vertical(|ui| {
                        ui.label(&probe.detail);
                        if !probe.remedy.is_empty() {
                            ui.label(
                                egui::RichText::new(&probe.remedy)
                                    .italics()
                                    .color(egui::Color32::GRAY),
                            );
                        }
                    });
                    ui.end_row();
                }
            });
    }

    fn draw_tuning(&mut self, ui: &mut egui::Ui) {
        let lang = &self.config.language;
        ui.heading(tr(lang, Msg::SectionTuning));
        ui.label(tr(lang, Msg::TuningPresetsHint));
        ui.add_space(6.0);

        let active = matching_preset(&self.config);
        for (i, preset) in PRESETS.iter().enumerate() {
            let selected = active == Some(i);
            if ui
                .selectable_label(selected, format!("{} — {}", preset.name, preset.summary))
                .clicked()
            {
                apply_preset(&mut self.config, i);
                self.mark_dirty(true);
            }
        }

        ui.separator();
        if ui
            .checkbox(&mut self.config.show_hud, tr(lang, Msg::ShowHud))
            .changed()
        {
            self.mark_dirty(true);
        }
        if ui
            .checkbox(&mut self.config.show_overlay, tr(lang, Msg::ShowOverlay))
            .changed()
        {
            self.mark_dirty(true);
        }
        if ui
            .checkbox(&mut self.config.advanced_mode, tr(lang, Msg::AdvancedMode))
            .changed()
        {
            if !self.config.advanced_mode && self.section == Section::Log {
                self.section = Section::Status;
            }
            self.mark_dirty(true);
        }

        ui.horizontal(|ui| {
            ui.label(tr(lang, Msg::ThemeStoredLabel));
            let mut theme = themes::normalize_theme(&self.config.theme).to_string();
            egui::ComboBox::from_id_salt("theme")
                .selected_text(&theme)
                .show_ui(ui, |ui| {
                    for name in THEMES {
                        ui.selectable_value(&mut theme, (*name).to_string(), *name);
                    }
                });
            if theme != self.config.theme {
                self.config.theme = theme;
                apply_theme(ui.ctx(), &self.config.theme);
                self.mark_dirty(true);
            }
        });

        ui.horizontal(|ui| {
            ui.label(tr(lang, Msg::LanguageLabel));
            let mut language = self.config.language.clone();
            egui::ComboBox::from_id_salt("language")
                .selected_text(&language)
                .show_ui(ui, |ui| {
                    for tag in ["en", "ru"] {
                        ui.selectable_value(&mut language, tag.to_string(), tag);
                    }
                });
            if language != self.config.language {
                self.config.language = language;
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(
                    tr(&self.config.language, Msg::AppTitle).to_string(),
                ));
                self.mark_dirty(true);
            }
        });

        ui.horizontal(|ui| {
            ui.label("Neural strength:");
            for strength in NeuralStrength::ALL {
                let selected = sidecar_config::neural_strength_of(&self.config) == Some(strength);
                if ui.selectable_label(selected, strength.name()).clicked() {
                    sidecar_config::set_neural_strength(&mut self.config, strength);
                    self.mark_dirty(true);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Flow grid:");
            for size in [1u32, 2, 4] {
                if ui
                    .selectable_label(self.config.flow_grid_size == size, size.to_string())
                    .clicked()
                {
                    self.config.flow_grid_size = size;
                    self.mark_dirty(true);
                }
            }
        });

        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tr(lang, Msg::SaveSettings)).clicked() {
                self.save_settings();
            }
            if ui.button(tr(lang, Msg::ReloadFromDisk)).clicked() {
                let (cfg, warnings) = load_config(&self.config_path);
                self.config = cfg;
                apply_theme(ui.ctx(), &self.config.theme);
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(
                    tr(&self.config.language, Msg::AppTitle).to_string(),
                ));
                self.saved_snapshot = self.config.clone();
                self.mark_dirty(false);
                for w in warnings {
                    self.log(format!("reload: {w}"));
                }
                self.refresh_probes();
            }
            if ui.button("Reset rendering defaults").clicked() {
                reset_rendering_settings(&mut self.config);
                self.mark_dirty(true);
            }
        });
        if self.dirty {
            ui.colored_label(egui::Color32::YELLOW, tr(lang, Msg::UnsavedChanges));
        }
    }

    fn draw_log(&mut self, ui: &mut egui::Ui) {
        ui.heading(tr(&self.config.language, Msg::SectionLog));
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for line in &self.log_lines {
                    ui.label(line);
                }
            });
    }
}

impl eframe::App for ManagerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_scan();
        self.poll_live_if_due(ctx);

        egui::SidePanel::left("nav")
            .resizable(false)
            .default_width(200.0)
            .show(ctx, |ui| self.draw_nav(ui));

        egui::CentralPanel::default().show(ctx, |ui| match self.section {
            Section::Status => self.draw_status(ui),
            Section::Setup => self.draw_setup(ui),
            Section::Checks => self.draw_checks(ui),
            Section::Tuning => self.draw_tuning(ui),
            Section::Log => self.draw_log(ui),
        });
    }
}
