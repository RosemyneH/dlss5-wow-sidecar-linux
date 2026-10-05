use std::time::{Duration, Instant};

use anyhow::Result;
use sidecar_capture::{
    capture_doctor_report, capture_error_remediation, format_doctor_line,
    parse_capture_node_from_env, start_capture, start_mock_stream, wow_window_hint,
    CaptureDoctorLevel, CaptureDoctorLine, CaptureError, CaptureFrame, FrameStream,
    ENV_CAPTURE_HINT, ENV_CAPTURE_NODE,
};

pub struct CaptureTestOptions {
    pub frames: u32,
    pub any_window: bool,
    pub timeout: Duration,
    pub mock: bool,
    pub verbose: bool,
}

#[derive(Debug, Default)]
pub struct CaptureSummary {
    pub requested: u32,
    pub captured: u32,
    pub invalid: u32,
    pub resolutions: Vec<(u32, u32)>,
    pub elapsed: Duration,
}

impl CaptureSummary {
    pub fn new(requested: u32) -> Self {
        Self {
            requested,
            ..Self::default()
        }
    }

    pub fn record(&mut self, frame: &CaptureFrame) -> bool {
        if !frame.validate() {
            self.invalid += 1;
            return false;
        }
        self.captured += 1;
        let res = (frame.width, frame.height);
        if self.resolutions.last() != Some(&res) {
            self.resolutions.push(res);
        }
        true
    }

    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.resolutions.last().copied()
    }

    pub fn resized(&self) -> bool {
        self.resolutions.len() > 1
    }

    pub fn fps(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if self.captured < 2 || secs <= 0.0 {
            return 0.0;
        }
        f64::from(self.captured - 1) / secs
    }

    pub fn complete(&self) -> bool {
        self.invalid == 0 && self.captured == self.requested
    }

    pub fn lines(&self) -> Vec<String> {
        let resolution = match self.resolution() {
            Some((w, h)) => format!("{w}x{h}"),
            None => "n/a".into(),
        };
        let mut out = vec![
            format!("frames:     {}/{}", self.captured, self.requested),
            format!("resolution: {resolution}"),
        ];
        if self.resized() {
            let chain: Vec<String> = self
                .resolutions
                .iter()
                .map(|(w, h)| format!("{w}x{h}"))
                .collect();
            out.push(format!("resized:    {}", chain.join(" -> ")));
        }
        if self.invalid > 0 {
            out.push(format!("invalid:    {}", self.invalid));
        }
        out.push(format!(
            "elapsed:    {:.2}s ({:.1} fps)",
            self.elapsed.as_secs_f64(),
            self.fps()
        ));
        out
    }
}

pub fn fixes_for_error(
    err: &CaptureError,
    captured: u32,
    node_override: Option<u32>,
) -> Vec<String> {
    let mut fixes = vec![capture_error_remediation(err).to_string()];
    match err {
        CaptureError::NotWayland(_) => {
            fixes.push("Log into a Wayland compositor (Hyprland/Sway); X11 sessions cannot use ScreenCast.".into());
            fixes.push("Verify offline with `wowsidecar-linux capture-test --mock`.".into());
        }
        CaptureError::Portal(_) => {
            fixes.push("Restart the portal: `systemctl --user restart xdg-desktop-portal xdg-desktop-portal-hyprland`.".into());
        }
        CaptureError::PipeWire(_) => {
            fixes.push("Restart PipeWire: `systemctl --user restart pipewire wireplumber`.".into());
        }
        CaptureError::PwRecord(_) => {
            fixes.push(
                "Install xdg-desktop-portal plus a compositor backend so the portal path succeeds."
                    .into(),
            );
        }
        CaptureError::Unavailable(_) if captured == 0 => {
            fixes.push(
                "Approve the ScreenCast dialog (it may be behind WoW or on another workspace)."
                    .into(),
            );
            fixes.push("Raise the wait with `--timeout <secs>` if picking takes longer.".into());
        }
        CaptureError::Unavailable(_) => {
            fixes.push("Stream stalled mid-capture: keep WoW visible; compositors pause hidden or minimized windows.".into());
        }
        CaptureError::StreamClosed => {
            fixes.push(
                "The shared window or portal session ended; reopen WoW and rerun capture-test."
                    .into(),
            );
        }
    }
    if let Some(node) = node_override {
        fixes.push(format!(
            "{ENV_CAPTURE_NODE}={node} is set; `unset {ENV_CAPTURE_NODE}` if that node is stale."
        ));
    }
    fixes
}

pub fn fix_for_doctor_line(line: &CaptureDoctorLine) -> Option<String> {
    if line.level == CaptureDoctorLevel::Ok {
        return None;
    }
    let fix = match line.label.as_str() {
        "wayland session" => "Log into a Wayland session (Hyprland/Sway) or export WAYLAND_DISPLAY.",
        "session dbus" => {
            "Launch from your desktop session or `export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus`."
        }
        "pipewire" => {
            "Install pipewire + wireplumber, then `systemctl --user enable --now pipewire wireplumber`."
        }
        "xdg-desktop-portal" => {
            "Portal is often in /usr/lib (not PATH); confirm with `systemctl --user status xdg-desktop-portal`."
        }
        "hyprland portal" => "Install xdg-desktop-portal-hyprland for per-window ScreenCast.",
        _ => return Some(format!("{}: {}", line.label, line.detail)),
    };
    Some(fix.to_string())
}

pub fn doctor_fixes() -> Vec<String> {
    capture_doctor_report()
        .iter()
        .filter_map(fix_for_doctor_line)
        .collect()
}

pub fn run(opts: &CaptureTestOptions) -> Result<()> {
    let node_override = parse_capture_node_from_env();
    let stream = match open_stream(opts, node_override) {
        Ok(s) => s,
        Err(e) => {
            print_failure(&e, &CaptureSummary::new(opts.frames), node_override);
            return Err(e.into());
        }
    };

    let mut summary = CaptureSummary::new(opts.frames);
    let started = Instant::now();
    let mut failure = None;
    for i in 0..opts.frames {
        match stream.next_frame_timeout(opts.timeout) {
            Ok(frame) => {
                let valid = summary.record(&frame);
                if opts.verbose || !valid {
                    println!(
                        "frame {i}: {}x{} rgba_bytes={} timestamp={}{}",
                        frame.width,
                        frame.height,
                        frame.rgba.len(),
                        frame.timestamp,
                        if valid { "" } else { " INVALID" }
                    );
                }
            }
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    summary.elapsed = started.elapsed();

    if let Some(e) = failure {
        print_failure(&e, &summary, node_override);
        return Err(e.into());
    }

    println!("\ncapture-test summary:");
    for line in summary.lines() {
        println!("  {line}");
    }
    if !summary.complete() {
        println!("\nfixes:");
        println!("  - Frame buffers did not match width*height*4; the negotiated format may be unsupported (try another source or report with `RUST_LOG=sidecar_capture=debug`).");
        anyhow::bail!(
            "capture-test: {} invalid frame(s) out of {}",
            summary.invalid,
            summary.requested
        );
    }
    if summary.resized() {
        println!("  note: source resolution changed during capture (window resized?)");
    }
    println!("capture-test: OK");
    Ok(())
}

fn open_stream(
    opts: &CaptureTestOptions,
    node_override: Option<u32>,
) -> Result<FrameStream, CaptureError> {
    if opts.mock {
        println!("mock capture: synthetic frames, portal/PipeWire skipped");
        return Ok(start_mock_stream());
    }
    if let Some(node) = node_override {
        println!(
            "{ENV_CAPTURE_NODE}={node} — portal picker skipped when this PipeWire node is alive"
        );
    } else {
        println!("portal: approve the ScreenCast dialog when it appears (Window tab on Hyprland)");
    }

    let hint = if opts.any_window {
        None
    } else {
        wow_window_hint()
    };
    match &hint {
        Some(h) => println!(
            "hint: [{}] {} ({}) — pick this surface in the portal if offered",
            h.compositor, h.title, h.address
        ),
        None if !opts.any_window => {
            println!("no WoW window hint; use --any-window or set {ENV_CAPTURE_HINT}")
        }
        None => {}
    }
    start_capture(hint)
}

fn print_failure(err: &CaptureError, summary: &CaptureSummary, node_override: Option<u32>) {
    eprintln!("\ncapture-test FAILED: {err}");
    for line in summary.lines() {
        eprintln!("  {line}");
    }
    eprintln!("\nfixes:");
    for fix in fixes_for_error(err, summary.captured, node_override) {
        eprintln!("  - {fix}");
    }
    let doctor: Vec<String> = capture_doctor_report()
        .iter()
        .filter(|l| l.level != CaptureDoctorLevel::Ok)
        .map(|l| {
            format!(
                "{}  => {}",
                format_doctor_line(l),
                fix_for_doctor_line(l).unwrap_or_default()
            )
        })
        .collect();
    if !doctor.is_empty() {
        eprintln!("\nenvironment issues:");
        for line in doctor {
            eprintln!("  {line}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidecar_capture::{mock_frame, MOCK_STREAM_H, MOCK_STREAM_W};

    #[test]
    fn summary_tracks_count_and_resolution() {
        let mut s = CaptureSummary::new(3);
        for i in 0..3 {
            assert!(s.record(&mock_frame(i)));
        }
        assert!(s.complete());
        assert_eq!(s.resolution(), Some((MOCK_STREAM_W, MOCK_STREAM_H)));
        assert!(!s.resized());
        let text = s.lines().join("\n");
        assert!(text.contains("frames:     3/3"));
        assert!(text.contains(&format!("{MOCK_STREAM_W}x{MOCK_STREAM_H}")));
    }

    #[test]
    fn summary_flags_invalid_and_resize() {
        let mut s = CaptureSummary::new(3);
        s.record(&CaptureFrame::new(vec![0; 16], 2, 2));
        s.record(&CaptureFrame::new(vec![0; 36], 3, 3));
        assert!(!s.record(&CaptureFrame::new(vec![0; 3], 1, 1)));
        assert_eq!(s.captured, 2);
        assert_eq!(s.invalid, 1);
        assert!(s.resized());
        assert!(!s.complete());
        assert!(s.lines().iter().any(|l| l.contains("2x2 -> 3x3")));
    }

    #[test]
    fn empty_summary_reports_na() {
        let s = CaptureSummary::new(5);
        assert!(s.lines().iter().any(|l| l.contains("0/5")));
        assert!(s.lines().iter().any(|l| l.contains("n/a")));
        assert_eq!(s.fps(), 0.0);
    }

    #[test]
    fn timeout_fixes_depend_on_progress() {
        let err = CaptureError::Unavailable("timed out".into());
        let before = fixes_for_error(&err, 0, None).join("\n");
        let during = fixes_for_error(&err, 4, None).join("\n");
        assert!(before.contains("--timeout"));
        assert!(during.contains("stalled"));
    }

    #[test]
    fn node_override_adds_unset_fix() {
        let fixes = fixes_for_error(&CaptureError::StreamClosed, 0, Some(42));
        assert!(fixes
            .iter()
            .any(|f| f.contains(&format!("unset {ENV_CAPTURE_NODE}"))));
    }

    #[test]
    fn doctor_fix_skips_ok_lines() {
        let ok = CaptureDoctorLine {
            level: CaptureDoctorLevel::Ok,
            label: "pipewire".into(),
            detail: "/usr/bin/pw-dump".into(),
        };
        let warn = CaptureDoctorLine {
            level: CaptureDoctorLevel::Warn,
            label: "pipewire".into(),
            detail: "missing".into(),
        };
        assert!(fix_for_doctor_line(&ok).is_none());
        assert!(fix_for_doctor_line(&warn).unwrap().contains("wireplumber"));
    }

    #[test]
    fn mock_stream_yields_valid_frames() {
        let stream = start_mock_stream();
        let mut s = CaptureSummary::new(2);
        for _ in 0..2 {
            s.record(&stream.next_frame_timeout(Duration::from_secs(2)).unwrap());
        }
        assert!(s.complete());
    }
}
