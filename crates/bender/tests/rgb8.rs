use bender::{BendEffect, BendState, PixelOrder};

#[test]
fn rgb_matches_rgba_and_bgra_through_stateful_sequences() {
    for seed in [0, 42, 987654321] {
        for stages in [
            &[][..],
            &["hold"][..],
            &["sensor"][..],
            &["isp"][..],
            &["mosh"][..],
            &["feedback"][..],
            &["sensor", "isp", "tape", "sync", "mosh", "feedback"][..],
        ] {
            let mut effect = BendEffect {
                seed,
                ..BendEffect::default()
            };
            for stage in stages {
                effect
                    .set_by_id(&format!("{stage}.enabled"), "true")
                    .unwrap();
            }
            for order in [PixelOrder::Rgba, PixelOrder::Bgra] {
                let mut rgb_state = BendState::default();
                let mut rgba_state = BendState::default();
                // Reuse state across size changes as well as successive frames.
                for (width, height) in [(1, 1), (17, 9), (64, 48), (65, 33)] {
                    for (input_pad, output_pad, omit_final_padding) in
                        [(0, 0, false), (5, 7, false), (1, 11, true)]
                    {
                        let input_stride = width * 3 + input_pad;
                        let output_stride = width * 3 + output_pad;
                        let rgba_stride = width * 4 + 3;
                        for (step, frame) in [0, 1, 1, 2, 9, 8, 9, 10].into_iter().enumerate() {
                            if step == 7 {
                                rgb_state.reset();
                                rgba_state.reset();
                            }
                            let mut input = vec![0xa5; input_stride * height];
                            let mut rgba = vec![0xa5; rgba_stride * height];
                            for y in 0..height {
                                for x in 0..width {
                                    let color = [
                                        (x * 37 + y * 11 + frame * 17) as u8,
                                        (x * 7 + y * 53 + frame * 29) as u8,
                                        (x * 19 + y * 3 + frame * 41) as u8,
                                    ];
                                    input[y * input_stride + x * 3..][..3].copy_from_slice(&color);
                                    let pixel = &mut rgba[y * rgba_stride + x * 4..][..4];
                                    let (ri, bi) = match order {
                                        PixelOrder::Rgba => (0, 2),
                                        PixelOrder::Bgra => (2, 0),
                                    };
                                    pixel[ri] = color[0];
                                    pixel[1] = color[1];
                                    pixel[bi] = color[2];
                                    pixel[3] = (x + y) as u8;
                                }
                            }
                            let output_len = output_stride * height
                                - if omit_final_padding { output_pad } else { 0 };
                            if omit_final_padding {
                                input.truncate(input_stride * height - input_pad);
                            }
                            let original = input.clone();
                            // Include a trailing guard after the declared image.
                            let mut output = vec![0x5a; output_len + 13];
                            let mut expected = vec![0x5a; rgba_stride * height];
                            effect.apply_rgb8(
                                width,
                                height,
                                &input,
                                input_stride,
                                &mut output[..output_len],
                                output_stride,
                                frame as u64,
                                &mut rgb_state,
                            );
                            effect.apply_rgba8(
                                width,
                                height,
                                &rgba,
                                rgba_stride,
                                &mut expected,
                                rgba_stride,
                                order,
                                frame as u64,
                                &mut rgba_state,
                            );
                            assert_eq!(input, original);
                            for y in 0..height {
                                for x in 0..width {
                                    let pixel = &expected[y * rgba_stride + x * 4..][..4];
                                    let color = match order {
                                        PixelOrder::Rgba => [pixel[0], pixel[1], pixel[2]],
                                        PixelOrder::Bgra => [pixel[2], pixel[1], pixel[0]],
                                    };
                                    assert_eq!(
                                        &output[y * output_stride + x * 3..][..3],
                                        &color,
                                        "seed={seed} stages={stages:?} frame={frame} x={x} y={y}"
                                    );
                                    assert_eq!(pixel[3], 255);
                                }
                                let pad_start = y * output_stride + width * 3;
                                let pad_end = ((y + 1) * output_stride).min(output_len);
                                assert!(output[pad_start..pad_end].iter().all(|&b| b == 0x5a));
                            }
                            assert!(output[output_len..].iter().all(|&b| b == 0x5a));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn rgb_empty_frames_leave_output_untouched() {
    let effect = BendEffect::default();
    let mut state = BendState::default();
    let mut output = [0x5a; 7];
    for (width, height) in [(0, 0), (0, 9), (9, 0)] {
        effect.apply_rgb8(width, height, &[], 0, &mut output, 0, 0, &mut state);
        assert_eq!(output, [0x5a; 7]);
    }
}

#[test]
fn rgb_rejects_invalid_layouts_before_writing() {
    // width, height, input length/stride, output length/stride
    for (width, height, input_len, input_stride, output_len, output_stride) in [
        (2, 2, 12, 5, 12, 6),
        (2, 2, 12, 6, 12, 5),
        (2, 2, 11, 6, 12, 6),
        (2, 2, 12, 6, 11, 6),
        (2, 2, 12, 6, 0, 6),
        (usize::MAX, 1, 0, usize::MAX, 0, usize::MAX),
        (1, 3, 0, usize::MAX, 0, 3),
    ] {
        let input = vec![0; input_len];
        let mut output = vec![0x5a; output_len];
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            BendEffect::default().apply_rgb8(
                width,
                height,
                &input,
                input_stride,
                &mut output,
                output_stride,
                0,
                &mut BendState::default(),
            );
        }));
        assert!(result.is_err());
        assert!(output.iter().all(|&b| b == 0x5a));
    }
}
