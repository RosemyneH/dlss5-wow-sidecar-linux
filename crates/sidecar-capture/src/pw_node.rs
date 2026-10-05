use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use pipewire as pw;
use pw::spa;
use pw::types::ObjectType;
use tracing::debug;

use crate::hint::{hyprland_addresses_equal, WindowHint};

pub const ENV_CAPTURE_AUTO_NODE: &str = "WOWSIDECAR_CAPTURE_AUTO_NODE";

const SCORE_ADDRESS: u32 = 10_000;
const SCORE_TITLE_EXACT: u32 = 600;
const SCORE_TITLE_CONTAINS: u32 = 500;
const SCORE_CLASS: u32 = 400;
const MIN_FUZZY_LEN: usize = 3;
const REGISTRY_SCAN_TIMEOUT: Duration = Duration::from_secs(2);

const TITLE_KEYS: &[&str] = &[
    "node.description",
    "node.name",
    "media.name",
    "window.title",
    "application.name",
    "app.name",
];
const CLASS_KEYS: &[&str] = &[
    "application.name",
    "app.name",
    "app.id",
    "application.id",
    "node.name",
    "window.class",
];
const CAMERA_APIS: &[&str] = &["v4l2", "libcamera"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PwNodeCandidate {
    pub id: u32,
    pub props: Vec<(String, String)>,
}

pub fn capture_auto_node_enabled() -> bool {
    match std::env::var(ENV_CAPTURE_AUTO_NODE).ok().as_deref() {
        None => true,
        Some(v) => !matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        ),
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
    let scored: Vec<(u32, u32)> = candidates
        .iter()
        .map(|n| (n.id, hint_match_score(hint, &n.props)))
        .filter(|(_, score)| *score > 0)
        .collect();

    if let Some(top) = scored.iter().map(|(_, s)| *s).max() {
        let tied: Vec<u32> = scored
            .iter()
            .filter(|(_, s)| *s == top)
            .map(|(id, _)| *id)
            .collect();
        if tied.len() == 1 || top >= SCORE_ADDRESS {
            let id = tied.into_iter().max()?;
            debug!(
                node_id = id,
                score = top,
                "auto-selected PipeWire capture node from registry"
            );
            return Some(id);
        }
        debug!(
            score = top,
            nodes = ?tied,
            "ambiguous PipeWire node match; set WOWSIDECAR_CAPTURE_NODE"
        );
        return None;
    }

    if !hint_has_window_identity(hint) {
        return None;
    }
    let mut screencasts = candidates.iter().filter(|n| is_screencast_node(&n.props));
    match (screencasts.next(), screencasts.next()) {
        (Some(only), None) => {
            debug!(
                node_id = only.id,
                "auto-selected sole PipeWire screencast node in session"
            );
            Some(only.id)
        }
        _ => None,
    }
}

pub fn hint_match_score(hint: &WindowHint, props: &[(String, String)]) -> u32 {
    if !is_video_capture_node(props) {
        return 0;
    }

    let mut score = 0;

    if props
        .iter()
        .filter(|(k, _)| !is_numeric_id_key(k))
        .any(|(_, v)| value_has_address_token(hint, v))
    {
        score += SCORE_ADDRESS;
    }

    let title = hint.title.trim().to_ascii_lowercase();
    if !title.is_empty() {
        let values = || TITLE_KEYS.iter().filter_map(|k| prop_value(props, k));
        if values().any(|v| v.trim().eq_ignore_ascii_case(&title)) {
            score += SCORE_TITLE_EXACT;
        } else if title.len() >= MIN_FUZZY_LEN
            && values().any(|v| v.to_ascii_lowercase().contains(&title))
        {
            score += SCORE_TITLE_CONTAINS;
        }
    }

    let class = hint.class.trim().to_ascii_lowercase();
    if class.len() >= MIN_FUZZY_LEN
        && CLASS_KEYS
            .iter()
            .filter_map(|k| prop_value(props, k))
            .any(|v| v.to_ascii_lowercase().contains(&class))
    {
        score += SCORE_CLASS;
    }

    score
}

fn value_has_address_token(hint: &WindowHint, value: &str) -> bool {
    let address = hint.address.trim();
    if address.is_empty() {
        return false;
    }
    let hypr = !hint.compositor.eq_ignore_ascii_case("sway");
    value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .any(|token| {
            if hypr {
                hyprland_addresses_equal(token, address)
            } else {
                token == address
            }
        })
}

fn is_numeric_id_key(key: &str) -> bool {
    let k = key.to_ascii_lowercase();
    k.starts_with("object.")
        || k.starts_with("client.")
        || k.starts_with("factory.")
        || k.ends_with(".id")
        || k.ends_with(".serial")
        || k == "media.class"
}

fn hint_has_window_identity(hint: &WindowHint) -> bool {
    !hint.address.is_empty() || !hint.title.is_empty() || !hint.class.is_empty()
}

pub fn is_video_capture_node(props: &[(String, String)]) -> bool {
    if is_camera_node(props) {
        return false;
    }

    let class_lc = prop_value(props, "media.class")
        .unwrap_or("")
        .to_ascii_lowercase();
    if class_lc.contains("audio") && !class_lc.contains("video") {
        return false;
    }
    if class_lc.contains("stream/input") {
        return false;
    }
    if class_lc.contains("video")
        && (class_lc.contains("source")
            || class_lc.contains("capture")
            || class_lc.contains("stream"))
    {
        return true;
    }

    let media_type = prop_value(props, "media.type").unwrap_or("");
    let media_category = prop_value(props, "media.category").unwrap_or("");
    let media_role = prop_value(props, "media.role").unwrap_or("");
    if media_type.eq_ignore_ascii_case("Video")
        && (media_category.eq_ignore_ascii_case("Capture")
            || media_role.eq_ignore_ascii_case("Screen"))
    {
        return true;
    }

    let name = prop_value(props, "node.name").unwrap_or("");
    let desc = prop_value(props, "node.description").unwrap_or("");
    let blob = format!("{name} {desc}").to_ascii_lowercase();
    ["xdpw", "xdph", "screen-capture", "screencast", "portal"]
        .iter()
        .any(|needle| blob.contains(needle))
}

fn is_camera_node(props: &[(String, String)]) -> bool {
    let api = prop_value(props, "device.api").unwrap_or("");
    let role = prop_value(props, "media.role").unwrap_or("");
    let name = prop_value(props, "node.name")
        .unwrap_or("")
        .to_ascii_lowercase();
    CAMERA_APIS.iter().any(|a| api.eq_ignore_ascii_case(a))
        || role.eq_ignore_ascii_case("Camera")
        || name.starts_with("v4l2_")
        || name.starts_with("libcamera")
}

fn is_screencast_node(props: &[(String, String)]) -> bool {
    is_video_capture_node(props) && prop_value(props, "device.api").is_none()
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

    let nodes: Rc<RefCell<Vec<PwNodeCandidate>>> = Rc::new(RefCell::new(Vec::new()));
    let done = Rc::new(Cell::new(false));
    let timed_out = Rc::new(Cell::new(false));
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
            let props = global.props.map(props_from_dict).unwrap_or_default();
            if is_video_capture_node(&props) {
                nodes_clone.borrow_mut().push(PwNodeCandidate {
                    id: global.id,
                    props,
                });
            }
        })
        .register();

    let timed_out_clone = timed_out.clone();
    let loop_clone = mainloop.clone();
    let timer = mainloop.loop_().add_timer(move |_| {
        timed_out_clone.set(true);
        loop_clone.quit();
    });
    timer.update_timer(Some(REGISTRY_SCAN_TIMEOUT), None);

    while !done.get() && !timed_out.get() {
        mainloop.run();
    }

    if !done.get() {
        return Err(format!(
            "registry scan timed out after {REGISTRY_SCAN_TIMEOUT:?}"
        ));
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

    fn props(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn node(id: u32, pairs: &[(&str, &str)]) -> PwNodeCandidate {
        PwNodeCandidate {
            id,
            props: props(pairs),
        }
    }

    fn hint(compositor: &str, address: &str, title: &str, class: &str) -> WindowHint {
        WindowHint {
            compositor: compositor.into(),
            address: address.into(),
            title: title.into(),
            class: class.into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        }
    }

    fn hypr_wow() -> WindowHint {
        hint("hyprland", "0x55d1c2a3b4e0", "World of Warcraft", "wow.exe")
    }

    fn sway_wow() -> WindowHint {
        hint("sway", "42", "World of Warcraft", "wow.exe")
    }

    fn xdph_stream(id: u32, desc: &str) -> PwNodeCandidate {
        node(
            id,
            &[
                ("media.class", "Video/Source"),
                ("node.name", "xdph-streaming-0"),
                ("node.description", desc),
                ("object.serial", "1423"),
            ],
        )
    }

    fn xdpw_stream(id: u32, serial: &str) -> PwNodeCandidate {
        node(
            id,
            &[
                ("media.class", "Video/Source"),
                ("node.name", "xdpw-stream"),
                ("object.serial", serial),
                ("client.id", "42"),
            ],
        )
    }

    fn webcam(id: u32) -> PwNodeCandidate {
        node(
            id,
            &[
                ("media.class", "Video/Source"),
                ("device.api", "v4l2"),
                ("media.role", "Camera"),
                ("node.name", "v4l2_input.pci-0000_00_14.0-usb-0_1_1.0"),
                ("node.description", "Integrated Camera"),
            ],
        )
    }

    fn obs_consumer(id: u32) -> PwNodeCandidate {
        node(
            id,
            &[
                ("media.class", "Stream/Input/Video"),
                ("node.name", "OBS"),
                ("node.description", "World of Warcraft"),
            ],
        )
    }

    #[test]
    fn video_capture_node_by_media_class() {
        assert!(is_video_capture_node(&props(&[
            ("media.class", "Video/Source"),
            ("node.name", "xdpw-screen-capture"),
        ])));
    }

    #[test]
    fn rejects_audio_sink() {
        assert!(!is_video_capture_node(&props(&[
            ("media.class", "Audio/Sink"),
            ("node.name", "alsa_output.foo"),
        ])));
    }

    #[test]
    fn rejects_webcam_and_stream_consumers() {
        assert!(!is_video_capture_node(&webcam(1).props));
        assert!(!is_video_capture_node(&obs_consumer(2).props));
    }

    #[test]
    fn pick_node_by_embedded_hypr_address() {
        let candidates = [
            xdph_stream(10, "window:0x55d1c2a3b4e0:World of Warcraft"),
            xdph_stream(11, "window:0x55d1c2a3ffff:World of Warcraft"),
        ];
        assert_eq!(
            pick_capture_node_from_candidates(&hypr_wow(), &candidates),
            Some(10)
        );
    }

    #[test]
    fn hypr_address_matches_without_prefix_and_case() {
        let candidates = [xdph_stream(12, "toplevel 55D1C2A3B4E0")];
        assert_eq!(
            pick_capture_node_from_candidates(&hypr_wow(), &candidates),
            Some(12)
        );
    }

    #[test]
    fn hypr_address_requires_whole_token() {
        let h = hint("hyprland", "0xb4e0", "", "");
        assert!(!value_has_address_token(&h, "window:0x55d1c2a3b4e0"));
        assert!(value_has_address_token(&h, "window:0xb4e0"));
    }

    #[test]
    fn empty_address_never_scores_as_address_match() {
        let h = hint("hyprland", "", "Diablo", "");
        let score = hint_match_score(&h, &xdph_stream(1, "World of Warcraft").props);
        assert_eq!(score, 0);
    }

    #[test]
    fn sway_con_id_ignores_serials_and_substrings() {
        let candidates = [xdpw_stream(20, "1423"), xdpw_stream(21, "4200")];
        for c in &candidates {
            assert!(hint_match_score(&sway_wow(), &c.props) < SCORE_ADDRESS);
        }
        let tagged = node(
            22,
            &[
                ("media.class", "Video/Source"),
                ("node.name", "xdpw-stream"),
                ("node.description", "sway con 42"),
            ],
        );
        assert!(hint_match_score(&sway_wow(), &tagged.props) >= SCORE_ADDRESS);
    }

    #[test]
    fn pick_node_by_title_when_no_address_in_metadata() {
        let h = hint("hyprland", "", "World of Warcraft", "");
        let candidates = [
            node(
                77,
                &[
                    ("media.type", "Video"),
                    ("media.category", "Capture"),
                    ("node.description", "Screen capture — World of Warcraft"),
                ],
            ),
            xdph_stream(78, "Firefox"),
        ];
        assert_eq!(pick_capture_node_from_candidates(&h, &candidates), Some(77));
    }

    #[test]
    fn exact_title_beats_substring_title() {
        let h = hint("hyprland", "", "World of Warcraft", "");
        let candidates = [
            xdph_stream(30, "World of Warcraft Launcher"),
            xdph_stream(31, "World of Warcraft"),
        ];
        assert_eq!(pick_capture_node_from_candidates(&h, &candidates), Some(31));
    }

    #[test]
    fn title_plus_class_beats_title_alone() {
        let h = hint("hyprland", "", "World of Warcraft", "wow.exe");
        let candidates = [
            xdph_stream(40, "World of Warcraft"),
            node(
                41,
                &[
                    ("media.class", "Video/Source"),
                    ("node.description", "World of Warcraft"),
                    ("application.name", "wow.exe"),
                ],
            ),
        ];
        assert_eq!(pick_capture_node_from_candidates(&h, &candidates), Some(41));
    }

    #[test]
    fn ambiguous_title_tie_yields_none() {
        let h = hint("hyprland", "", "World of Warcraft", "");
        let candidates = [
            xdph_stream(50, "World of Warcraft"),
            xdph_stream(51, "World of Warcraft"),
        ];
        assert_eq!(pick_capture_node_from_candidates(&h, &candidates), None);
    }

    #[test]
    fn address_tie_prefers_newest_node() {
        let candidates = [
            xdph_stream(60, "window:0x55d1c2a3b4e0"),
            xdph_stream(61, "window:0x55d1c2a3b4e0"),
        ];
        assert_eq!(
            pick_capture_node_from_candidates(&hypr_wow(), &candidates),
            Some(61)
        );
    }

    #[test]
    fn webcam_named_like_wow_is_never_picked() {
        let h = hint("hyprland", "", "Integrated Camera", "");
        assert_eq!(pick_capture_node_from_candidates(&h, &[webcam(70)]), None);
    }

    #[test]
    fn sole_screencast_node_when_only_one_in_session() {
        let candidates = [webcam(4), xdpw_stream(5, "900")];
        assert_eq!(
            pick_capture_node_from_candidates(&hypr_wow(), &candidates),
            Some(5)
        );
    }

    #[test]
    fn sole_fallback_skips_multiple_screencasts() {
        let candidates = [xdpw_stream(5, "900"), xdpw_stream(6, "901")];
        assert_eq!(
            pick_capture_node_from_candidates(&hypr_wow(), &candidates),
            None
        );
    }

    #[test]
    fn sole_fallback_requires_window_identity() {
        let h = hint("hyprland", "", "", "");
        assert_eq!(
            pick_capture_node_from_candidates(&h, &[xdpw_stream(5, "900")]),
            None
        );
    }

    #[test]
    fn short_class_does_not_fuzzy_match() {
        let h = hint("sway", "", "", "wo");
        assert_eq!(hint_match_score(&h, &xdph_stream(1, "x").props), 0);
    }

    #[test]
    fn capture_auto_node_env_toggle() {
        std::env::set_var(ENV_CAPTURE_AUTO_NODE, "0");
        assert!(!capture_auto_node_enabled());
        std::env::set_var(ENV_CAPTURE_AUTO_NODE, "OFF");
        assert!(!capture_auto_node_enabled());
        std::env::set_var(ENV_CAPTURE_AUTO_NODE, "yes");
        assert!(capture_auto_node_enabled());
        std::env::remove_var(ENV_CAPTURE_AUTO_NODE);
        assert!(capture_auto_node_enabled());
    }
}
