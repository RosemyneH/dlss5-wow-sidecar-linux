use std::os::fd::OwnedFd;
use std::sync::mpsc::Sender;
use std::thread::{self, JoinHandle};

use ashpd::desktop::screencast::{
    CursorMode, Screencast, SelectSourcesOptions, SourceType, Stream as PortalStream,
};
use ashpd::desktop::PersistMode;
use pipewire as pw;
use pw::{properties::properties, spa};
use tracing::{info, warn};

use crate::error::CaptureError;
use crate::frame::CaptureFrame;
use crate::hint::WindowHint;

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

        if let Err(e) = runtime.block_on(run_portal_capture(hint, frame_tx)) {
            warn!(error = %e, "portal capture ended");
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
        info!("portal capture: select a window in the portal dialog");
    }

    let (portal_stream, fd) = open_portal().await?;
    let node_id = portal_stream.pipe_wire_node_id();
    start_pipewire(node_id, fd, frame_tx)
}

async fn open_portal() -> Result<(PortalStream, OwnedFd), CaptureError> {
    let proxy = Screencast::new()
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?;
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?;

    proxy
        .select_sources(
            &session,
            SelectSourcesOptions::default()
                .set_cursor_mode(CursorMode::Metadata)
                .set_sources(SourceType::Monitor | SourceType::Window)
                .set_multiple(false)
                .set_restore_token(None)
                .set_persist_mode(PersistMode::DoNot),
        )
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?;

    let response = proxy
        .start(&session, None, Default::default())
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?
        .response()
        .map_err(|e| CaptureError::Portal(e.to_string()))?;

    let stream = response
        .streams()
        .first()
        .cloned()
        .ok_or_else(|| CaptureError::Portal("no stream selected in portal dialog".into()))?;

    let fd = proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await
        .map_err(|e| CaptureError::Portal(e.to_string()))?;

    Ok((stream, fd))
}

fn start_pipewire(
    node_id: u32,
    fd: OwnedFd,
    frame_tx: Sender<Result<CaptureFrame, CaptureError>>,
) -> Result<(), CaptureError> {
    pw::init();

    let mainloop = pw::main_loop::MainLoopBox::new(None)
        .map_err(|e| CaptureError::PipeWire(e.to_string()))?;
    let context = pw::context::ContextBox::new(mainloop.loop_(), None)
        .map_err(|e| CaptureError::PipeWire(e.to_string()))?;
    let core = context
        .connect_fd(fd, None)
        .map_err(|e| CaptureError::PipeWire(e.to_string()))?;

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
