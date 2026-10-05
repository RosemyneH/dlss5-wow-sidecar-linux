use std::process::Command;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DesktopWindow {
    pub compositor: String,
    pub address: String,
    pub title: String,
    pub class: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

#[derive(Deserialize)]
struct HyprClient {
    address: String,
    title: String,
    class: String,
    at: [i32; 2],
    size: [u32; 2],
    fullscreen: u8,
}

fn is_non_game_wow_related_window(title: &str, class: &str) -> bool {
    let t = title.to_ascii_lowercase();
    let c = class.to_ascii_lowercase();
    t.contains("sidecar")
        || t.contains("wowsidecar")
        || c.contains("wowsidecar")
        || (t.contains("dlss") && t.contains("sidecar"))
}

/// Higher = more likely the live game client (Proton / native).
pub fn wow_window_match_score(title: &str, class: &str) -> u32 {
    if is_non_game_wow_related_window(title, class) {
        return 0;
    }
    let t = title.to_ascii_lowercase();
    let c = class.to_ascii_lowercase();
    let mut score = 0u32;
    if c.contains("wow.exe") || c == "wow" {
        score += 1000;
    }
    if c.contains("gxwindow") {
        score += 900;
    }
    if t.starts_with("world of warcraft") {
        score += 800;
    } else if t.contains("world of warcraft") {
        score += 600;
    }
    if c.contains("wine") && (t.contains("wow") || t.contains("world of warcraft")) {
        score += 500;
    }
    if t.contains("wow") && !t.contains("sidecar") {
        score += 200;
    }
    score
}

fn title_or_class_looks_like_wow(title: &str, class: &str) -> bool {
    wow_window_match_score(title, class) > 0
}

pub fn list_wow_windows() -> Vec<DesktopWindow> {
    let mut out = Vec::new();
    out.extend(hyprland_wow_windows());
    out.extend(sway_wow_windows());
    out.sort_by(|a, b| {
        wow_window_match_score(&b.title, &b.class)
            .cmp(&wow_window_match_score(&a.title, &a.class))
            .then_with(|| a.title.cmp(&b.title))
    });
    out
}

fn hyprland_wow_windows() -> Vec<DesktopWindow> {
    let output = match Command::new("hyprctl").args(["clients", "-j"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let clients: Vec<HyprClient> = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    clients
        .into_iter()
        .filter(|c| title_or_class_looks_like_wow(&c.title, &c.class))
        .map(|c| DesktopWindow {
            compositor: "hyprland".into(),
            address: c.address,
            title: c.title,
            class: c.class,
            x: c.at[0],
            y: c.at[1],
            width: c.size[0],
            height: c.size[1],
            fullscreen: c.fullscreen != 0,
        })
        .collect()
}

#[derive(Deserialize)]
struct SwayNode {
    id: Option<u64>,
    name: Option<String>,
    #[serde(rename = "app_id")]
    app_id: Option<String>,
    rect: Option<SwayRect>,
    nodes: Option<Vec<SwayNode>>,
    floating_nodes: Option<Vec<SwayNode>>,
}

#[derive(Deserialize)]
struct SwayRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn sway_wow_windows() -> Vec<DesktopWindow> {
    let output = match Command::new("swaymsg").args(["-t", "get_tree"]).output() {
        Ok(o) => o,
        Err(_) => return Vec::new(),
    };
    if !output.status.success() {
        return Vec::new();
    }
    let root: SwayNode = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    let mut found = Vec::new();
    walk_sway(&root, &mut found);
    found
}

fn walk_sway(node: &SwayNode, out: &mut Vec<DesktopWindow>) {
    let title = node.name.as_deref().unwrap_or("");
    let class = node.app_id.as_deref().unwrap_or("");
    if let Some(id) = node.id {
        if title_or_class_looks_like_wow(title, class) {
            if let Some(rect) = &node.rect {
                out.push(DesktopWindow {
                    compositor: "sway".into(),
                    address: id.to_string(),
                    title: title.to_string(),
                    class: class.to_string(),
                    x: rect.x,
                    y: rect.y,
                    width: rect.width.max(0) as u32,
                    height: rect.height.max(0) as u32,
                    fullscreen: false,
                });
            }
        }
    }
    for child in node.nodes.iter().flatten() {
        walk_sway(child, out);
    }
    for child in node.floating_nodes.iter().flatten() {
        walk_sway(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidecar_manager_title_is_not_a_wow_window() {
        assert_eq!(
            wow_window_match_score("DLSS 5 — Sidecar for World of Warcraft — v0.4.0", "some-ui",),
            0
        );
    }

    #[test]
    fn proton_wow_client_scores_highest() {
        let game = wow_window_match_score("World of Warcraft", "wow.exe");
        let sidecar =
            wow_window_match_score("DLSS 5 — Sidecar for World of Warcraft", "wowsidecar");
        assert!(game > sidecar);
        assert!(game >= 1000);
    }
}
