use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::thread;

use eframe::egui;
use sidecar_config::{
    apply_preset, default_config_path, load_config, matching_preset, reset_rendering_settings,
    save_config, sidecar_dir, Config, NeuralStrength, PRESETS,
};
use sidecar_core::{list_wow_windows, smart_scan_installs, SmartScanOptions, WowInstall};
use sidecar_probes::{run_all_probes, ProbeResult, ProbeState};

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
        Box::new(|cc| Ok(Box::new(ManagerApp::new(cc)))),
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
    fn label(self) -> &'static str {
        match self {
            Section::Status => "Status",
            Section::Setup => "Setup",
            Section::Checks => "Checks",
            Section::Tuning => "Tuning",
            Section::Log => "Log",
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
}

impl ManagerApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let config_path = default_config_path();
        let (config, warnings) = load_config(&config_path);
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
        self.log("probes refreshed");
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
                self.status_message = "Settings saved.".to_string();
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
            ui.heading("WoW Sidecar");
            ui.label(format!("v{}", env!("CARGO_PKG_VERSION")));
            ui.add_space(8.0);
            for sec in Section::ALL {
                if sec == Section::Log && !self.config.advanced_mode {
                    continue;
                }
                let selected = self.section == sec;
                let mut label = sec.label().to_string();
                if sec == Section::Tuning && self.dirty {
                    label.push('*');
                }
                if ui.selectable_label(selected, label).clicked() {
                    self.section = sec;
                }
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.label(format!("Language: en (fixed for now)"));
                ui.label(format!("Theme: {}", self.config.theme));
            });
        });
    }

    fn draw_status(&mut self, ui: &mut egui::Ui) {
        ui.heading("Status");
        ui.label(
            "Out-of-process sidecar for Linux (Hyprland / Sway). The overlay runtime is not \
             wired yet; this panel loads settings and runs safety checks.",
        );
        ui.add_space(8.0);
        if !self.status_message.is_empty() {
            ui.colored_label(egui::Color32::LIGHT_GREEN, &self.status_message);
        }
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
        let windows = list_wow_windows();
        ui.label(format!("WoW windows: {}", windows.len()));
        ui.label(format!(
            "Installs on disk (last scan): {}",
            self.installs.len()
        ));

        let blocked = self.probes.iter().any(|p| p.state == ProbeState::Fail);
        if blocked {
            ui.colored_label(
                egui::Color32::LIGHT_RED,
                "Some checks failed. Open Checks before expecting capture to work.",
            );
        } else {
            ui.colored_label(egui::Color32::LIGHT_GREEN, "No blocking check failures.");
        }
    }

    fn draw_setup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Setup");
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
                .max_height(220.0)
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

        ui.add_space(12.0);
        ui.label("WoW game folder (for injector scan only — never install the sidecar here):");
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
        ui.heading("Checks");
        ui.horizontal(|ui| {
            if ui.button("Refresh probes").clicked() {
                self.refresh_probes();
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
        ui.heading("Tuning");
        ui.label(
            "Presets adjust rendering settings only. Interface language stays English for now.",
        );
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
        if ui.checkbox(&mut self.config.show_hud, "Show HUD").changed() {
            self.mark_dirty(true);
        }
        if ui
            .checkbox(&mut self.config.show_overlay, "Show overlay")
            .changed()
        {
            self.mark_dirty(true);
        }
        if ui
            .checkbox(&mut self.config.advanced_mode, "Advanced mode")
            .changed()
        {
            if !self.config.advanced_mode && self.section == Section::Log {
                self.section = Section::Status;
            }
            self.mark_dirty(true);
        }

        ui.horizontal(|ui| {
            ui.label("Theme (stored; styling TBD):");
            let mut theme = self.config.theme.clone();
            egui::ComboBox::from_id_salt("theme")
                .selected_text(&theme)
                .show_ui(ui, |ui| {
                    for name in ["stormwind", "questlog", "dragonflight"] {
                        ui.selectable_value(&mut theme, name.to_string(), name);
                    }
                });
            if theme != self.config.theme {
                self.config.theme = theme;
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
            if ui.button("Save settings").clicked() {
                self.save_settings();
            }
            if ui.button("Reload from disk").clicked() {
                let (cfg, warnings) = load_config(&self.config_path);
                self.config = cfg;
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
            ui.colored_label(egui::Color32::YELLOW, "Unsaved changes.");
        }
    }

    fn draw_log(&mut self, ui: &mut egui::Ui) {
        ui.heading("Log");
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

        if self.dirty && self.config != self.saved_snapshot {
            // keep dirty flag in sync if user edited via presets etc.
        }
    }
}
