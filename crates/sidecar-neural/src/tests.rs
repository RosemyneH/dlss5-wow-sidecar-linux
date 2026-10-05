use super::*;

const TEST_W: u32 = 64;
const TEST_H: u32 = 64;

fn test_layout() -> FrameLayout {
    FrameLayout::new(TEST_W, TEST_H).expect("64x64 layout")
}

fn checker_rgba(layout: &FrameLayout) -> Vec<u8> {
    let mut buf = vec![0u8; layout.byte_len()];
    for y in 0..layout.height {
        for x in 0..layout.width {
            let i = layout.pixel_offset(x, y);
            let v = if (x + y) % 2 == 0 { 40 } else { 200 };
            buf[i] = v;
            buf[i + 1] = v / 2;
            buf[i + 2] = 255 - v;
            buf[i + 3] = 255;
        }
    }
    buf
}

#[test]
fn passthrough_64x64_identity() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let mut output = vec![0u8; layout.byte_len()];
    let mut proc = Passthrough;
    proc.process(&layout, &input, &mut output).unwrap();
    assert_eq!(input, output);
}

#[test]
fn sharpen_64x64_changes_edges() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let mut passthrough_out = vec![0u8; layout.byte_len()];
    let mut sharpen_out = vec![0u8; layout.byte_len()];
    Passthrough
        .process(&layout, &input, &mut passthrough_out)
        .unwrap();
    let mut sharpen = SimpleSharpen::new(0.85).unwrap();
    sharpen.process(&layout, &input, &mut sharpen_out).unwrap();
    assert_ne!(passthrough_out, sharpen_out);
    assert_eq!(sharpen_out.len(), layout.byte_len());
}

#[test]
fn onnx_backend_not_wired_yet() {
    let model = std::env::temp_dir().join("wowsidecar-neural-test.onnx");
    std::fs::write(&model, b"stub").unwrap();
    let result = build_processor(NeuralBackend::Onnx { model_path: model });
    assert!(matches!(result, Err(ProcessError::OnnxUnavailable)));
}

#[test]
fn process_frame_triple_pass_differs_from_single() {
    use sidecar_config::Config;

    let layout = test_layout();
    let input = checker_rgba(&layout);
    let mut once = vec![0u8; layout.byte_len()];
    let mut thrice = vec![0u8; layout.byte_len()];

    let cfg_once = Config {
        neural_passes: 1,
        ..Config::default()
    };
    process_frame(&cfg_once, &layout, &input, &mut once).unwrap();

    let cfg_thrice = Config {
        neural_passes: 3,
        ..Config::default()
    };
    process_frame(&cfg_thrice, &layout, &input, &mut thrice).unwrap();

    assert_ne!(once, thrice);
}

#[test]
fn config_maps_reshade_to_sharpen_id() {
    use sidecar_config::Config;

    let mut cfg = Config::default();
    assert_eq!(processor_id_for_config(&cfg), "simple_sharpen");
    cfg.neural_pass = "passthrough".into();
    assert_eq!(processor_id_for_config(&cfg), "passthrough");
}

fn gradient_rgba(layout: &FrameLayout) -> Vec<u8> {
    let mut buf = vec![0u8; layout.byte_len()];
    for y in 0..layout.height {
        for x in 0..layout.width {
            let i = layout.pixel_offset(x, y);
            let v = (96 + (x * 3 + y) % 64 + if (x / 8 + y / 8) % 2 == 0 { 8 } else { 0 }) as u8;
            buf[i..i + 4].copy_from_slice(&[v, v / 2 + 40, 200 - v / 2, 255]);
        }
    }
    buf
}

fn preset_config(index: usize) -> sidecar_config::Config {
    let mut cfg = sidecar_config::Config::default();
    sidecar_config::apply_preset(&mut cfg, index);
    cfg
}

fn chain_by_hand(amount: f32, passes: u32, layout: &FrameLayout, input: &[u8]) -> Vec<u8> {
    let mut sharpen = SimpleSharpen::new(amount).unwrap();
    let mut cur = input.to_vec();
    let mut next = vec![0u8; layout.byte_len()];
    for _ in 0..passes {
        sharpen.process(layout, &cur, &mut next).unwrap();
        std::mem::swap(&mut cur, &mut next);
    }
    cur
}

#[test]
fn build_processor_from_config_matches_windows_presets() {
    let expected = [
        ("simple_sharpen", Some(1.0)),
        ("simple_sharpen", Some(0.60)),
        ("simple_sharpen", Some(0.85 * 0.92)),
        ("passthrough", None),
    ];
    assert_eq!(sidecar_config::PRESETS.len(), expected.len());
    for (i, (id, amount)) in expected.into_iter().enumerate() {
        let cfg = preset_config(i);
        assert_eq!(build_processor_from_config(&cfg).unwrap().id(), id);
        assert_eq!(processor_id_for_config(&cfg), id);
        match (neural_backend_from_config(&cfg), amount) {
            (NeuralBackend::SimpleSharpen { amount: got }, Some(want)) => {
                assert!((got - want).abs() < 1e-6, "preset {i}: {got} != {want}")
            }
            (NeuralBackend::Passthrough, None) => {}
            (other, _) => panic!("preset {i}: unexpected backend {other:?}"),
        }
    }
}

#[test]
fn build_processor_from_config_onnx_missing_model() {
    let cfg = sidecar_config::Config {
        neural_pass: "onnx".into(),
        wow_dir: "/nonexistent/wowsidecar-neural".into(),
        ..sidecar_config::Config::default()
    };
    assert_eq!(processor_id_for_config(&cfg), "onnx");
    assert!(matches!(
        build_processor_from_config(&cfg),
        Err(ProcessError::OnnxModelMissing(_))
    ));
}

#[test]
fn neural_pass_count_clamps_to_one_through_four() {
    let mut cfg = sidecar_config::Config::default();
    for (raw, want) in [
        (0, 1),
        (1, 1),
        (2, 2),
        (3, 3),
        (4, 4),
        (9, MAX_NEURAL_PASSES),
    ] {
        cfg.neural_passes = raw;
        assert_eq!(neural_pass_count(&cfg), want);
    }
}

#[test]
fn process_frame_pass_counts_one_through_four() {
    let layout = test_layout();
    let input = gradient_rgba(&layout);
    let mut outputs = Vec::new();
    for passes in 1..=4 {
        let cfg = sidecar_config::Config {
            neural_passes: passes,
            ..preset_config(0)
        };
        let mut out = vec![0u8; layout.byte_len()];
        process_frame(&cfg, &layout, &input, &mut out).unwrap();
        assert_eq!(
            out,
            chain_by_hand(1.0, passes, &layout, &input),
            "{passes} passes"
        );
        assert!(!outputs.contains(&out), "{passes} passes duplicated output");
        outputs.push(out);
    }
}

#[test]
fn process_frame_off_preset_is_identity_at_four_passes() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let cfg = sidecar_config::Config {
        neural_passes: 4,
        ..preset_config(3)
    };
    let mut out = vec![0u8; layout.byte_len()];
    process_frame(&cfg, &layout, &input, &mut out).unwrap();
    assert_eq!(out, input);
}

#[test]
fn process_frame_rejects_mismatched_buffers() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let mut out = vec![0u8; layout.byte_len() - 4];
    assert!(matches!(
        process_frame(
            &sidecar_config::Config::default(),
            &layout,
            &input,
            &mut out
        ),
        Err(ProcessError::BufferSize { .. })
    ));
}

#[test]
fn chain_reload_unchanged_config_is_noop() {
    let cfg = preset_config(0);
    let mut chain = NeuralChain::from_config(&cfg).unwrap();
    assert!(!chain.reload(&cfg).unwrap());
    assert_eq!(chain.spec(), &NeuralChainSpec::from_config(&cfg));
}

#[test]
fn chain_reload_applies_preset_and_pass_changes() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let mut chain = NeuralChain::from_config(&preset_config(0)).unwrap();

    let stronger = sidecar_config::Config {
        neural_passes: 3,
        ..preset_config(0)
    };
    assert!(chain.reload(&stronger).unwrap());
    assert_eq!(chain.spec().passes, 3);
    let mut out = vec![0u8; layout.byte_len()];
    chain.process(&layout, &input, &mut out).unwrap();
    assert_eq!(out, chain_by_hand(1.0, 3, &layout, &input));

    assert!(chain.reload(&preset_config(3)).unwrap());
    assert_eq!(chain.processor_id(), "passthrough");
    chain.process(&layout, &input, &mut out).unwrap();
    assert_eq!(out, input);

    assert!(chain.reload(&preset_config(2)).unwrap());
    assert_eq!(chain.processor_id(), "simple_sharpen");
    chain.process(&layout, &input, &mut out).unwrap();
    assert_eq!(out, chain_by_hand(0.85 * 0.92, 1, &layout, &input));
}

#[test]
fn chain_matches_process_frame_across_reloads() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let mut chain = NeuralChain::from_config(&preset_config(0)).unwrap();
    for index in [1, 2, 0, 3, 1] {
        for passes in 1..=4 {
            let cfg = sidecar_config::Config {
                neural_passes: passes,
                ..preset_config(index)
            };
            chain.reload(&cfg).unwrap();
            let mut cached = vec![0u8; layout.byte_len()];
            let mut fresh = vec![0u8; layout.byte_len()];
            chain.process(&layout, &input, &mut cached).unwrap();
            process_frame(&cfg, &layout, &input, &mut fresh).unwrap();
            assert_eq!(cached, fresh, "preset {index}, {passes} passes");
        }
    }
}

#[test]
fn chain_reload_failure_keeps_previous_chain() {
    let layout = test_layout();
    let input = checker_rgba(&layout);
    let cfg = preset_config(0);
    let mut chain = NeuralChain::from_config(&cfg).unwrap();
    let broken = sidecar_config::Config {
        neural_pass: "onnx".into(),
        wow_dir: "/nonexistent/wowsidecar-neural".into(),
        ..cfg.clone()
    };
    assert!(chain.reload(&broken).is_err());
    assert_eq!(chain.spec(), &NeuralChainSpec::from_config(&cfg));
    let mut out = vec![0u8; layout.byte_len()];
    chain.process(&layout, &input, &mut out).unwrap();
    assert_eq!(out, chain_by_hand(1.0, 1, &layout, &input));
}
