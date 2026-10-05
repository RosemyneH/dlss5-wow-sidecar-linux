use sidecar_capture::synthetic_frame_4x4;
use sidecar_neural::{build_processor, run_pipeline, FrameProcessor, NeuralBackend, Passthrough};

#[test]
fn mock_capture_passthrough_then_sharpen_4x4_without_portal() {
    let frame = synthetic_frame_4x4();
    assert!(frame.validate());

    let passthrough = build_processor(NeuralBackend::Passthrough).expect("passthrough");
    let sharpen = build_processor(NeuralBackend::SimpleSharpen { amount: 0.75 }).expect("sharpen");

    let mut stages: Vec<Box<dyn sidecar_neural::FrameProcessor>> = vec![passthrough, sharpen];

    let out = run_pipeline(frame.width, frame.height, &frame.rgba, &mut stages).expect("pipeline");

    assert_eq!(out.len(), frame.rgba.len());
    assert_ne!(
        out, frame.rgba,
        "sharpen stage should change synthetic pattern"
    );

    let identity = {
        let mut only_pass = vec![build_processor(NeuralBackend::Passthrough).unwrap()];
        run_pipeline(frame.width, frame.height, &frame.rgba, &mut only_pass).unwrap()
    };
    assert_eq!(identity, frame.rgba);
}

#[test]
fn passthrough_alone_is_identity_on_mock_frame() {
    let frame = synthetic_frame_4x4();
    let mut proc = Passthrough;
    let layout = sidecar_neural::FrameLayout::new(frame.width, frame.height).expect("layout");
    let mut out = vec![0u8; layout.byte_len()];
    proc.process(&layout, &frame.rgba, &mut out).unwrap();
    assert_eq!(out, frame.rgba);
}
