use egui::{Color32, Context, Visuals};

pub const THEMES: &[&str] = &["stormwind", "questlog", "dragonflight"];

pub fn normalize_theme(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "questlog" => "questlog",
        "dragonflight" => "dragonflight",
        _ => "stormwind",
    }
}

pub fn apply_theme(ctx: &Context, theme: &str) {
    ctx.set_visuals(visuals_for(normalize_theme(theme)));
}

fn visuals_for(theme: &str) -> Visuals {
    match theme {
        "questlog" => questlog(),
        "dragonflight" => dragonflight(),
        _ => stormwind(),
    }
}

fn stormwind() -> Visuals {
    let mut v = Visuals::dark();
    v.window_fill = Color32::from_rgb(22, 34, 58);
    v.panel_fill = Color32::from_rgb(18, 28, 48);
    v.extreme_bg_color = Color32::from_rgb(12, 20, 36);
    v.faint_bg_color = Color32::from_rgb(36, 52, 86);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(30, 44, 72);
    v.widgets.inactive.bg_fill = Color32::from_rgb(38, 54, 88);
    v.widgets.hovered.bg_fill = Color32::from_rgb(48, 68, 108);
    v.widgets.active.bg_fill = Color32::from_rgb(72, 98, 148);
    v.selection.bg_fill = Color32::from_rgb(56, 96, 160);
    v.selection.stroke.color = Color32::from_rgb(200, 168, 78);
    v.hyperlink_color = Color32::from_rgb(160, 200, 255);
    v.warn_fg_color = Color32::from_rgb(240, 200, 90);
    v.error_fg_color = Color32::from_rgb(240, 110, 110);
    v.override_text_color = Some(Color32::from_rgb(225, 230, 240));
    v
}

fn questlog() -> Visuals {
    let mut v = Visuals::light();
    v.window_fill = Color32::from_rgb(244, 232, 205);
    v.panel_fill = Color32::from_rgb(236, 220, 188);
    v.extreme_bg_color = Color32::from_rgb(220, 200, 168);
    v.faint_bg_color = Color32::from_rgb(250, 242, 224);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(230, 214, 182);
    v.widgets.inactive.bg_fill = Color32::from_rgb(218, 198, 162);
    v.widgets.hovered.bg_fill = Color32::from_rgb(208, 186, 148);
    v.widgets.active.bg_fill = Color32::from_rgb(168, 132, 88);
    v.selection.bg_fill = Color32::from_rgb(140, 108, 68);
    v.selection.stroke.color = Color32::from_rgb(92, 64, 36);
    v.hyperlink_color = Color32::from_rgb(72, 48, 24);
    v.warn_fg_color = Color32::from_rgb(140, 90, 20);
    v.error_fg_color = Color32::from_rgb(160, 48, 32);
    v.override_text_color = Some(Color32::from_rgb(48, 32, 18));
    v
}

fn dragonflight() -> Visuals {
    let mut v = Visuals::dark();
    v.window_fill = Color32::from_rgb(14, 28, 30);
    v.panel_fill = Color32::from_rgb(10, 22, 24);
    v.extreme_bg_color = Color32::from_rgb(6, 14, 16);
    v.faint_bg_color = Color32::from_rgb(24, 42, 44);
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(20, 38, 40);
    v.widgets.inactive.bg_fill = Color32::from_rgb(28, 48, 50);
    v.widgets.hovered.bg_fill = Color32::from_rgb(36, 62, 64);
    v.widgets.active.bg_fill = Color32::from_rgb(48, 88, 86);
    v.selection.bg_fill = Color32::from_rgb(196, 92, 38);
    v.selection.stroke.color = Color32::from_rgb(61, 214, 195);
    v.hyperlink_color = Color32::from_rgb(61, 214, 195);
    v.warn_fg_color = Color32::from_rgb(255, 180, 80);
    v.error_fg_color = Color32::from_rgb(255, 96, 72);
    v.override_text_color = Some(Color32::from_rgb(220, 235, 232));
    v
}
