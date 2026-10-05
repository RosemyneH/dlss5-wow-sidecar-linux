use std::os::fd::OwnedFd;
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, Stream as PortalStream,
};
use ashpd::desktop::{PersistMode, ResponseError};
use ashpd::enumflags2::BitFlags;
use ashpd::PortalError;
use pipewire as pw;
use pw::{properties::properties, spa};
use tracing::{info, warn};

use crate::error::{enrich_portal_error, CaptureError};
use crate::frame::CaptureFrame;
use crate::hint::WindowHint;
use crate::portal_restore::{
    clear_screencast_restore_token, load_screencast_restore_token, save_screencast_restore_token,
};

pub struct PortalHandle {
    join: JoinHandle<()>,
}

impl PortalHandle {
    pub fn stop(self) {
        let _ = self.join.join();
    }
}

struct StreamUserData {
    format: spa::param::video::VideoInfoRaw,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
}

pub fn spawn_portal_stream(
    hint: Option<WindowHint>,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
) -> Result<PortalHandle, CaptureError> {
    let join = thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                let _ = frame_tx.send(Err(CaptureError::Portal(e.to_string())));
                return;
            }
        };

        if let Err(e) = runtime.block_on(run_portal_capture(hint, frame_tx.clone())) {
            let _ = frame_tx.send(Err(e));
            warn!("portal capture ended");
        }
    });

    Ok(PortalHandle { join })
}

async fn run_portal_capture(
    hint: Option<WindowHint>,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
) -> Result<(), CaptureError> {
    if let Some(h) = &hint {
        info!(
            title = %h.title,
            compositor = %h.compositor,
            address = %h.address,
            "portal capture: pick this window in the dialog when prompted"
        );
    } else {
        info!("portal capture: select a window in the portal dialog (Window tab on Hyprland)");
    }

    info!("portal: waiting for ScreenCast picker — approve sharing in the desktop dialog");
    let (portal_stream, fd) = open_portal(hint.as_ref()).await?;
    let node_id = portal_stream.pipe_wire_node_id();
    info!(
        node_id,
        "portal ScreenCast active — set WOWSIDECAR_CAPTURE_NODE={node_id} to skip the picker while this node exists"
    );
    start_pipewire(node_id, Some(fd), frame_tx)
}

pub fn spawn_direct_pipewire_stream(
    node_id: u32,
    hint: Option<WindowHint>,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
) -> Result<PortalHandle, CaptureError> {
    if let Some(h) = &hint {
        info!(
            node_id,
            title = %h.title,
            compositor = %h.compositor,
            address = %h.address,
            "direct PipeWire capture (WOWSIDECAR_CAPTURE_NODE); portal dialog skipped"
        );
    } else {
        info!(
            node_id,
            "direct PipeWire capture (WOWSIDECAR_CAPTURE_NODE); portal dialog skipped"
        );
    }

    let join = thread::spawn(move || {
        if let Err(e) = start_pipewire(node_id, None, frame_tx.clone()) {
            let _ = frame_tx.send(Err(e));
            warn!("direct pipewire capture ended");
        }
    });

    Ok(PortalHandle { join })
}

const MISSING_BACKEND: &str = "no ScreenCast implementation on the session bus — install/start xdg-desktop-portal plus the compositor backend (Hyprland: xdg-desktop-portal-hyprland, Sway/wlroots: xdg-desktop-portal-wlr, KDE: xdg-desktop-portal-kde, GNOME: xdg-desktop-portal-gnome); `wowsidecar-linux doctor` lists what is missing";

async fn open_portal(hint: Option<&WindowHint>) -> Result<(PortalStream, OwnedFd), CaptureError> {
    let saved = load_screencast_restore_token();
    match open_portal_with_token(hint, saved.as_deref()).await {
        Ok(pair) => Ok(pair),
        Err(e) if saved.is_some() && restore_token_failure(&e) => {
            warn!("screencast restore token rejected; clearing saved token and retrying picker");
            clear_screencast_restore_token();
            open_portal_with_token(hint, None).await
        }
        Err(e) => Err(e),
    }
}

fn restore_token_failure(err: &CaptureError) -> bool {
    let msg = err.to_string().to_ascii_lowercase();
    msg.contains("restore") || msg.contains("invalid") || msg.contains("not allowed")
}

async fn open_portal_with_token(
    hint: Option<&WindowHint>,
    restore_token: Option<&str>,
) -> Result<(PortalStream, OwnedFd), CaptureError> {
    let proxy = Screencast::new()
        .await
        .map_err(|e| portal_err("connect", &e))?;
    let available = proxy
        .available_source_types()
        .await
        .map_err(|e| portal_err("query source types", &e))?;
    let sources = requested_sources(available)?;

    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|e| portal_err("create session", &e))?;

    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Metadata)
                .set_sources(sources)
                .set_multiple(false)
                .set_restore_token(restore_token)
                .set_persist_mode(PersistMode::ExplicitlyRevoked),
        )
        .await
        .map_err(|e| portal_err("select sources", &e))?;

    let response = proxy
        .start(&session, None, Default::default())
        .await
        .map_err(|e| portal_err("start", &e))?
        .response()
        .map_err(|e| portal_err("start", &e))?;

    if let Some(token) = response.restore_token() {
        save_screencast_restore_token(token);
        info!("screencast restore token saved for next session (picker may be skipped)");
    }

    let stream = pick_stream(response.streams(), hint)?;

    let fd = proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await
        .map_err(|e| portal_err("open PipeWire remote", &e))?;

    Ok((stream, fd))
}

fn requested_sources(
    available: BitFlags<SourceType>,
) -> Result<BitFlags<SourceType>, CaptureError> {
    let wanted = available & (SourceType::Monitor | SourceType::Window);
    if wanted.is_empty() {
        return Err(CaptureError::Portal(format!(
            "ScreenCast advertises no monitor or window sources (AvailableSourceTypes={available:?}) — {MISSING_BACKEND}"
        )));
    }
    if !wanted.contains(SourceType::Window) {
        warn!(
            "ScreenCast backend is monitor-only (e.g. xdg-desktop-portal-wlr) — pick the output that shows WoW; install xdg-desktop-portal-hyprland on Hyprland for per-window capture"
        );
    }
    Ok(wanted)
}

fn pick_stream(
    streams: &[PortalStream],
    hint: Option<&WindowHint>,
) -> Result<PortalStream, CaptureError> {
    let Some(stream) = streams.first() else {
        return Err(CaptureError::Portal(
            "portal returned no stream — sharing was denied or nothing was selected; run capture-test again and pick the WoW window".into(),
        ));
    };
    if streams.len() > 1 {
        warn!(
            count = streams.len(),
            "portal shared several sources; using the first"
        );
    }
    if stream.source_type() != Some(SourceType::Monitor) {
        return Ok(stream.clone());
    }

    let (Some(h), Some(pos), Some(size)) = (hint, stream.position(), stream.size()) else {
        info!("portal shared a whole monitor; the overlay will see the full output");
        return Ok(stream.clone());
    };
    if !window_on_monitor(h, pos, size) {
        return Err(CaptureError::Portal(format!(
            "multi-monitor: shared monitor at {},{} ({}x{}) does not contain the WoW window at {},{} ({}x{}) — run capture-test again and pick the WoW window, or the monitor it is on",
            pos.0, pos.1, size.0, size.1, h.x, h.y, h.width, h.height
        )));
    }
    info!(
        x = pos.0,
        y = pos.1,
        width = size.0,
        height = size.1,
        "portal shared the monitor containing WoW; picking the window instead avoids capturing other surfaces"
    );
    Ok(stream.clone())
}

fn window_on_monitor(hint: &WindowHint, pos: (i32, i32), size: (i32, i32)) -> bool {
    let cx = i64::from(hint.x) + i64::from(hint.width) / 2;
    let cy = i64::from(hint.y) + i64::from(hint.height) / 2;
    let (mx, my) = (i64::from(pos.0), i64::from(pos.1));
    cx >= mx && cx < mx + i64::from(size.0) && cy >= my && cy < my + i64::from(size.1)
}

fn portal_err(stage: &str, err: &ashpd::Error) -> CaptureError {
    CaptureError::Portal(format!("{stage}: {}", describe_portal_error(err)))
}

fn describe_portal_error(err: &ashpd::Error) -> String {
    match err {
        ashpd::Error::Response(ResponseError::Cancelled)
        | ashpd::Error::Portal(PortalError::Cancelled(_)) => format!(
            "{err} — ScreenCast was cancelled or denied in the dialog, nothing is shared; run capture-test again and pick the WoW window"
        ),
        ashpd::Error::Portal(PortalError::NotAllowed(_)) => format!(
            "{err} — ScreenCast denied by portal policy (permission store or sandbox); allow screen sharing for this app and retry"
        ),
        ashpd::Error::Response(ResponseError::Other) => format!(
            "{err} — the compositor portal backend refused the request; check `journalctl --user -u xdg-desktop-portal -u 'xdg-desktop-portal-*'`"
        ),
        ashpd::Error::NoResponse => format!(
            "{err} — the compositor portal backend never answered; restart xdg-desktop-portal and its backend"
        ),
        ashpd::Error::PortalNotFound(_) => format!("{err} — {MISSING_BACKEND}"),
        _ if err.to_string().contains("ServiceUnknown") => format!("{err} — {MISSING_BACKEND}"),
        _ => enrich_portal_error(&err.to_string()),
    }
}

fn start_pipewire(
    node_id: u32,
    portal_fd: Option<OwnedFd>,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
) -> Result<(), CaptureError> {
    pw::init();

    let mainloop =
        pw::main_loop::MainLoopBox::new(None).map_err(|e| CaptureError::PipeWire(e.to_string()))?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)
        .map_err(|e| CaptureError::PipeWire(e.to_string()))?;
    let core = match portal_fd {
        Some(fd) => context
            .connect_fd(fd, None)
            .map_err(|e| CaptureError::PipeWire(e.to_string()))?,
        None => context
            .connect(None)
            .map_err(|e| CaptureError::PipeWire(e.to_string()))?,
    };

    let user_data = StreamUserData {
        format: spa::param::video::VideoInfoRaw::default(),
        frame_tx,
    };

    let stream = pw::stream::StreamBox::new(
        &core,
        "sidecar-capture",
        properties! {
            *pw::keys::MEDIA_TYPE => "Video",
            *pw::keys::MEDIA_CATEGORY => "Capture",
            *pw::keys::MEDIA_ROLE => "Screen",
        },
    )
    .map_err(|e| CaptureError::PipeWire(e.to_string()))?;

    let _listener = stream
        .add_local_listener_with_user_data(user_data)
        .param_changed(|_, user_data, id, param| {
            let Some(param) = param else {
                return;
            };
            if id != pw::spa::param::ParamType::Format.as_raw() {
                return;
            }
            let (media_type, media_subtype) =
                match pw::spa::param::format_utils::parse_format(param) {
                    Ok(v) => v,
                    Err(_) => return,
                };
            if media_type != spa::param::format::MediaType::Video
                || media_subtype != spa::param::format::MediaSubtype::Raw
            {
                return;
            }
            let _ = user_data.format.parse(param);
        })
        .process(|stream, user_data| {
            let Some(mut buffer) = stream.dequeue_buffer() else {
                return;
            };
            let datas = buffer.datas_mut();
            if datas.is_empty() {
                return;
            }
            let data = &mut datas[0];
            let chunk_size = data.chunk().size() as usize;
            if chunk_size == 0 {
                return;
            }
            let Some(bytes) = data.data() else {
                return;
            };
            if let Some(frame) = raw_buffer_to_frame(&user_data.format, bytes) {
                let _ = user_data.frame_tx.send(Ok(frame));
            }
        })
        .register()
        .map_err(|e| CaptureError::PipeWire(e.to_string()))?;

    let obj = pw::spa::pod::object!(
        spa::utils::SpaTypes::ObjectParamFormat,
        spa::param::ParamType::EnumFormat,
        pw::spa::pod::property!(
            spa::param::format::FormatProperties::MediaType,
            Id,
            spa::param::format::MediaType::Video
        ),
        pw::spa::pod::property!(
            spa::param::format::FormatProperties::MediaSubtype,
            Id,
            spa::param::format::MediaSubtype::Raw
        ),
        pw::spa::pod::property!(
            spa::param::format::FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBA,
            spa::param::video::VideoFormat::RGBx,
            spa::param::video::VideoFormat::BGRx,
            spa::param::video::VideoFormat::RGB,
        ),
        pw::spa::pod::property!(
            spa::param::format::FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            spa::utils::Rectangle {
                width: 64,
                height: 64
            },
            spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            spa::utils::Rectangle {
                width: 7680,
                height: 4320
            }
        ),
    );

    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
    .map_err(|e| CaptureError::PipeWire(e.to_string()))?
    .0
    .into_inner();

    let pod = spa::pod::Pod::from_bytes(&values)
        .ok_or_else(|| CaptureError::PipeWire("failed to build format pod".into()))?;
    let mut params = [pod];

    stream
        .connect(
            spa::utils::Direction::Input,
            Some(node_id),
            pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
            &mut params,
        )
        .map_err(|e| CaptureError::PipeWire(e.to_string()))?;

    mainloop.run();
    Ok(())
}

fn raw_buffer_to_frame(
    format: &spa::param::video::VideoInfoRaw,
    bytes: &[u8],
) -> Option<CaptureFrame> {
    let width = format.size().width;
    let height = format.size().height;
    if width == 0 || height == 0 {
        return None;
    }

    let rgba = match format.format() {
        spa::param::video::VideoFormat::RGBA | spa::param::video::VideoFormat::RGBx => {
            let need = width as usize * height as usize * 4;
            if bytes.len() < need {
                return None;
            }
            let mut out = bytes[..need].to_vec();
            if format.format() == spa::param::video::VideoFormat::RGBx {
                for px in out.chunks_mut(4) {
                    px[3] = 255;
                }
            }
            out
        }
        spa::param::video::VideoFormat::BGRx => {
            let need = width as usize * height as usize * 4;
            if bytes.len() < need {
                return None;
            }
            let mut out = Vec::with_capacity(need);
            for px in bytes[..need].as_chunks::<4>().0 {
                out.extend_from_slice(&[px[2], px[1], px[0], 255]);
            }
            out
        }
        spa::param::video::VideoFormat::RGB => {
            let need = width as usize * height as usize * 3;
            if bytes.len() < need {
                return None;
            }
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for px in bytes[..need].as_chunks::<3>().0 {
                out.extend_from_slice(&[px[0], px[1], px[2], 255]);
            }
            out
        }
        _ => return None,
    };

    Some(CaptureFrame::new(rgba, width, height))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ashpd::desktop::screencast::StreamBuilder;

    fn wow_at(x: i32, y: i32) -> WindowHint {
        WindowHint {
            compositor: "hyprland".into(),
            address: "0x1".into(),
            title: "World of Warcraft".into(),
            class: "wow.exe".into(),
            x,
            y,
            width: 1920,
            height: 1080,
        }
    }

    fn monitor(x: i32, y: i32) -> PortalStream {
        StreamBuilder::new(42)
            .source_type(SourceType::Monitor)
            .position((x, y))
            .size((2560, 1440))
            .build()
    }

    #[test]
    fn cancelled_reads_as_denial() {
        let msg = describe_portal_error(&ashpd::Error::Response(ResponseError::Cancelled));
        assert!(msg.contains("cancelled or denied"));
    }

    #[test]
    fn missing_portal_names_backends() {
        let name = "org.freedesktop.portal.ScreenCast".try_into().unwrap();
        let msg = describe_portal_error(&ashpd::Error::PortalNotFound(name));
        assert!(msg.contains("xdg-desktop-portal-hyprland"));
    }

    #[test]
    fn no_sources_is_missing_backend() {
        let err = requested_sources(BitFlags::empty()).unwrap_err();
        assert!(err.to_string().contains("compositor backend"));
    }

    #[test]
    fn monitor_only_backend_still_requests_monitor() {
        let s = requested_sources(SourceType::Monitor.into()).unwrap();
        assert_eq!(s, BitFlags::from(SourceType::Monitor));
    }

    #[test]
    fn empty_streams_is_denial() {
        let err = pick_stream(&[], None).unwrap_err();
        assert!(err.to_string().contains("denied"));
    }

    #[test]
    fn wrong_monitor_is_rejected() {
        let err = pick_stream(&[monitor(2560, 0)], Some(&wow_at(100, 100))).unwrap_err();
        assert!(err.to_string().contains("multi-monitor"));
    }

    #[test]
    fn monitor_containing_wow_is_accepted() {
        let s = pick_stream(&[monitor(2560, 0)], Some(&wow_at(2700, 100))).unwrap();
        assert_eq!(s.pipe_wire_node_id(), 42);
    }

    #[test]
    fn window_stream_skips_geometry_check() {
        let w = StreamBuilder::new(7)
            .source_type(SourceType::Window)
            .build();
        assert!(pick_stream(&[w], Some(&wow_at(-9999, -9999))).is_ok());
    }
}
