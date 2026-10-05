use std::cell::Cell;
use std::rc::Rc;

use pipewire as pw;
use pw::spa;
use pw::types::ObjectType;
use tracing::debug;

use crate::hint::{identifier_matches_hint, WindowHint};

pub const ENV_CAPTURE_AUTO_NODE: &str = "WOWSIDECAR_CAPTURE_AUTO_NODE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PwNodeCandidate {
    pub id: u32,
    pub props: Vec<(String, String)>,
}

pub fn capture_auto_node_enabled() -> bool {
    match std::env::var(ENV_CAPTURE_AUTO_NODE).ok().as_deref() {
        None => true,
        Some("") => true,
        Some(v) => {
            let v = v.trim();
            !matches!(
                v.to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        }
    }
}

pub fn resolve_capture_node_id(hint: Option<&WindowHint>) -> Option<u32> {
    if let Some(id) = crate::hint::parse_capture_node_from_env() {
        return Some(id);
    }
    if !capture_auto_node_enabled() {
        return None;
    }
    hint.and_then(discover_capture_node_for_hint)
}

pub fn discover_capture_node_for_hint(hint: &WindowHint) -> Option<u32> {
    let candidates = match enumerate_video_capture_nodes() {
        Ok(nodes) => nodes,
        Err(e) => {
            debug!(error = %e, "PipeWire node scan failed");
            return None;
        }
    };
    pick_capture_node_from_candidates(hint, &candidates)
}

pub fn pick_capture_node_from_candidates(
    hint: &WindowHint,
    candidates: &[PwNodeCandidate],
) -> Option<u32> {
    let mut best: Option<(u32, u32)> = None;
    for node in candidates {
        let score = hint_match_score(hint, &node.props);
        if score == 0 {
            continue;
        }
        match best {
            None => best = Some((node.id, score)),
            Some((_, prev)) if score > prev => best = Some((node.id, score)),
            _ => {}
        }
    }

    if best.is_none()
        && candidates.len() == 1
        && hint_has_window_identity(hint)
        && is_video_capture_node(&candidates[0].props)
    {
        let id = candidates[0].id;
        debug!(
            node_id = id,
            "auto-selected sole PipeWire video capture node in session"
        );
        return Some(id);
    }

    best.map(|(id, score)| {
        debug!(
            node_id = id,
            score, "auto-selected PipeWire capture node from registry"
        );
        id
    })
}

pub fn hint_match_score(hint: &WindowHint, props: &[(String, String)]) -> u32 {
    if !is_video_capture_node(props) {
        return 0;
    }

    let mut score = 0u32;
    for (_, value) in props {
        if identifier_matches_hint(hint, value) {
            score = score.max(1000);
        }
    }

    if !hint.title.is_empty() {
        let title = hint.title.to_ascii_lowercase();
        for key in [
            "node.description",
            "node.name",
            "application.name",
            "app.name",
        ] {
            if let Some(v) = prop_value(props, key) {
                if v.to_ascii_lowercase().contains(&title) {
                    score = score.max(500);
                }
            }
        }
    }

    if !hint.class.is_empty() {
        let class = hint.class.to_ascii_lowercase();
        for key in ["application.name", "app.name", "node.name", "window.class"] {
            if let Some(v) = prop_value(props, key) {
                if v.to_ascii_lowercase().contains(&class) {
                    score = score.max(400);
                }
            }
        }
    }

    score
}

fn hint_has_window_identity(hint: &WindowHint) -> bool {
    !hint.address.is_empty() || !hint.title.is_empty() || !hint.class.is_empty()
}

pub fn is_video_capture_node(props: &[(String, String)]) -> bool {
    let media_class = prop_value(props, "media.class").unwrap_or("");
    let media_type = prop_value(props, "media.type").unwrap_or("");
    let media_category = prop_value(props, "media.category").unwrap_or("");
    let media_role = prop_value(props, "media.role").unwrap_or("");
    let name = prop_value(props, "node.name").unwrap_or("");
    let desc = prop_value(props, "node.description").unwrap_or("");

    let class_lc = media_class.to_ascii_lowercase();
    if class_lc.contains("audio") && !class_lc.contains("video") {
        return false;
    }

    if class_lc.contains("video")
        && (class_lc.contains("source")
            || class_lc.contains("capture")
            || class_lc.contains("stream"))
    {
        return true;
    }

    if media_type.eq_ignore_ascii_case("Video")
        && (media_category.eq_ignore_ascii_case("Capture")
            || media_role.eq_ignore_ascii_case("Screen"))
    {
        return true;
    }

    let blob = format!("{name} {desc}").to_ascii_lowercase();
    blob.contains("xdpw")
        || blob.contains("screen-capture")
        || blob.contains("screencast")
        || blob.contains("portal")
}

fn prop_value<'a>(props: &'a [(String, String)], key: &str) -> Option<&'a str> {
    props
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(key))
        .map(|(_, v)| v.as_str())
}

fn enumerate_video_capture_nodes() -> Result<Vec<PwNodeCandidate>, String> {
    pw::init();

    let mainloop = pw::main_loop::MainLoopRc::new(None).map_err(|e| format!("main loop: {e}"))?;
    let context =
        pw::context::ContextRc::new(&mainloop, None).map_err(|e| format!("context: {e}"))?;
    let core = context
        .connect_rc(None)
        .map_err(|e| format!("connect: {e}"))?;
    let registry = core.get_registry().map_err(|e| format!("registry: {e}"))?;

    let nodes: Rc<Cell<Vec<PwNodeCandidate>>> = Rc::new(Cell::new(Vec::new()));
    let done = Rc::new(Cell::new(false));
    let pending = core.sync(0).map_err(|e| format!("sync: {e}"))?;

    let done_clone = done.clone();
    let loop_clone = mainloop.clone();
    let _core_listener = core
        .add_listener_local()
        .done(move |id, seq| {
            if id == pw::core::PW_ID_CORE && seq == pending {
                done_clone.set(true);
                loop_clone.quit();
            }
        })
        .register();

    let nodes_clone = nodes.clone();
    let _registry_listener = registry
        .add_listener_local()
        .global(move |global| {
            if global.type_ != ObjectType::Node {
                return;
            }
            let props = global
                .props
                .as_ref()
                .map(|dict| props_from_dict(dict))
                .unwrap_or_default();
            if !is_video_capture_node(&props) {
                return;
            }
            let mut list = nodes_clone.take();
            list.push(PwNodeCandidate {
                id: global.id,
                props,
            });
            nodes_clone.set(list);
        })
        .register();

    while !done.get() {
        mainloop.run();
    }

    Ok(nodes.take())
}

fn props_from_dict(dict: &spa::utils::dict::DictRef) -> Vec<(String, String)> {
    dict.iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hint::WindowHint;

    fn sample_props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn hypr_hint() -> WindowHint {
        WindowHint {
            compositor: "hyprland".into(),
            address: "0xcafe".into(),
            title: "World of Warcraft".into(),
            class: "gxwindow".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }
    }

    #[test]
    fn video_capture_node_by_media_class() {
        let props = sample_props(&[
            ("media.class", "Video/Source"),
            ("node.name", "xdpw-screen-capture"),
        ]);
        assert!(is_video_capture_node(&props));
    }

    #[test]
    fn rejects_audio_sink() {
        let props = sample_props(&[
            ("media.class", "Audio/Sink"),
            ("node.name", "alsa_output.foo"),
        ]);
        assert!(!is_video_capture_node(&props));
    }

    #[test]
    fn pick_node_by_embedded_hypr_address() {
        let hint = hypr_hint();
        let candidates = [
            PwNodeCandidate {
                id: 10,
                props: sample_props(&[
                    ("media.class", "Video/Source"),
                    ("node.description", "window:0xcafe:World of Warcraft"),
                ]),
            },
            PwNodeCandidate {
                id: 11,
                props: sample_props(&[
                    ("media.class", "Video/Source"),
                    ("node.description", "other window"),
                ]),
            },
        ];
        assert_eq!(
            pick_capture_node_from_candidates(&hint, &candidates),
            Some(10)
        );
    }

    #[test]
    fn pick_node_by_title_when_no_address_in_metadata() {
        let hint = WindowHint {
            compositor: "hyprland".into(),
            address: String::new(),
            title: "World of Warcraft".into(),
            class: String::new(),
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let candidates = [PwNodeCandidate {
            id: 77,
            props: sample_props(&[
                ("media.type", "Video"),
                ("media.category", "Capture"),
                ("node.description", "Screen capture — World of Warcraft"),
            ]),
        }];
        assert_eq!(
            pick_capture_node_from_candidates(&hint, &candidates),
            Some(77)
        );
    }

    #[test]
    fn capture_auto_node_env_toggle() {
        std::env::set_var(ENV_CAPTURE_AUTO_NODE, "0");
        assert!(!capture_auto_node_enabled());
        std::env::set_var(ENV_CAPTURE_AUTO_NODE, "yes");
        assert!(capture_auto_node_enabled());
        std::env::remove_var(ENV_CAPTURE_AUTO_NODE);
    }

    #[test]
    fn sole_capture_node_when_only_one_in_session() {
        let hint = hypr_hint();
        let props = sample_props(&[
            ("media.class", "Video/Source"),
            ("node.name", "xdpw-screen-capture-source"),
        ]);
        let candidates = [PwNodeCandidate { id: 5, props }];
        assert_eq!(
            pick_capture_node_from_candidates(&hint, &candidates),
            Some(5)
        );
    }
}
