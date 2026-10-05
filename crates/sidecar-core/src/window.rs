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

fn title_or_class_looks_like_wow(title: &str, class: &str) -> bool {
    let t = title.to_ascii_lowercase();
    let c = class.to_ascii_lowercase();
    t.contains("world of warcraft")
        || t.contains("warcraft")
        || c.contains("wow")
        || c.contains("gxwindow")
        || c.contains("wine")
            && (t.contains("wow") || t.contains("warcraft"))
}

pub fn list_wow_windows() -> Vec<DesktopWindow> {
    let mut out = Vec::new();
    out.extend(hyprland_wow_windows());
    out.extend(sway_wow_windows());
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
    if node.id.is_some() && title_or_class_looks_like_wow(title, class) {
        if let Some(rect) = &node.rect {
            out.push(DesktopWindow {
                compositor: "sway".into(),
                address: node.id.unwrap().to_string(),
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
    for child in node.nodes.iter().flatten() {
        walk_sway(child, out);
    }
    for child in node.floating_nodes.iter().flatten() {
        walk_sway(child, out);
    }
}
