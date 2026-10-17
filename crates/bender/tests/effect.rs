use bender::settings::{Axis, BridgeMode, DataTarget, InvertMode, StuckMode};
use bender::{BendEffect, BendState, PixelOrder, PresetError, YiqPlanes};

const W: usize = 64;
const H: usize = 48;

fn gradient(seed: u8) -> Vec<u8> {
    let mut v = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let p = &mut v[(y * W + x) * 4..][..4];
            p[0] = (x * 4) as u8;
            p[1] = (y * 5) as u8;
            p[2] = ((x + y) * 2) as u8 ^ seed;
            p[3] = 255;
        }
    }
    v
}

fn grey_gradient() -> Vec<u8> {
    let mut v = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let g = ((x + y) * 255 / (W + H - 2)) as u8;
            v[(y * W + x) * 4..][..4].copy_from_slice(&[g, g, g, 255]);
        }
    }
    v
}

fn run(e: &BendEffect, src: &[u8], frame: u64, state: &mut BendState) -> Vec<u8> {
    let mut dst = vec![0u8; src.len()];
    e.apply_rgba8(
        W,
        H,
        src,
        W * 4,
        &mut dst,
        W * 4,
        PixelOrder::Rgba,
        frame,
        state,
    );
    dst
}

#[test]
fn default_is_identity() {
    let src = gradient(0);
    let out = run(&BendEffect::default(), &src, 0, &mut BendState::default());
    for (a, b) in src.iter().zip(&out) {
        assert!(a.abs_diff(*b) <= 1, "{a} vs {b}");
    }
}

#[test]
fn deterministic_per_frame() {
    let mut e = BendEffect::default();
    e.sync.enabled = true;
    e.sync.settings.h_jitter = 0.1;
    e.sync.settings.tear_rate = 1.0;
    let src = gradient(0);
    let a = run(&e, &src, 7, &mut BendState::default());
    let b = run(&e, &src, 7, &mut BendState::default());
    let c = run(&e, &src, 8, &mut BendState::default());
    assert_eq!(a, b);
    assert_ne!(a, c);
}

#[test]
fn address_stuck_low_tiles_columns() {
    let mut e = BendEffect::default();
    e.address_a.enabled = true;
    let s = &mut e.address_a.settings;
    s.axis = Axis::Column;
    s.bit = 3;
    s.mode = BridgeMode::StuckLow;
    s.coverage = 1.0;
    let mut src = vec![0u8; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let g = (x * 4) as u8;
            src[(y * W + x) * 4..][..4].copy_from_slice(&[g, g, g, 255]);
        }
    }
    let out = run(&e, &src, 0, &mut BendState::default());
    for y in 0..H {
        for x in 0..W {
            let o = &out[(y * W + x) * 4..][..3];
            let r = &out[(y * W + (x & !8)) * 4..][..3];
            assert_eq!(o, r, "x={x} y={y}");
        }
    }
}

#[test]
fn data_stuck_high_msb_brightens() {
    let mut e = BendEffect::default();
    e.data.enabled = true;
    let s = &mut e.data.settings;
    s.target = DataTarget::Luma;
    s.swap_a = 3;
    s.swap_b = 3;
    s.stuck_bit = 7;
    s.stuck_mode = StuckMode::High;
    s.coverage = 1.0;
    let out = run(&e, &grey_gradient(), 0, &mut BendState::default());
    for p in out.as_chunks::<4>().0 {
        let l =
            (0.299 * f32::from(p[0]) + 0.587 * f32::from(p[1]) + 0.114 * f32::from(p[2])) / 255.0;
        assert!(l >= 0.49, "luma {l}");
    }
}

#[test]
fn hold_continuity() {
    let mut e = BendEffect::default();
    e.hold.enabled = true;
    e.hold.settings.rate = 1.0;
    e.hold.settings.decay = 0.0;
    let f0 = gradient(0);
    let f1 = gradient(0x55);
    let f5 = gradient(0xaa);
    let mut state = BendState::default();
    let out0 = run(&e, &f0, 0, &mut state);
    let out1 = run(&e, &f1, 1, &mut state);
    assert_eq!(out1, out0);
    let out1b = run(&e, &f1, 1, &mut state);
    assert_eq!(out1b, out0);
    let out5 = run(&e, &f5, 5, &mut state);
    assert_eq!(out5, run(&e, &f5, 5, &mut BendState::default()));
    assert_ne!(out5, out0);
}

#[test]
fn bleed_delays_and_trails_chroma_only() {
    let mut planes = YiqPlanes::new(W, H);
    for r in 0..H {
        planes.y[r * W..(r + 1) * W].fill(0.5);
        planes.i[r * W + 16] = 0.4;
        planes.q[r * W + 16] = -0.2;
    }
    let mut e = BendEffect::default();
    e.bleed.enabled = true;
    e.bleed.settings.shift = 4.0 / W as f32;
    e.bleed.settings.smear = 0.05;
    e.apply_yiq(&mut planes, 0, &mut BendState::default());

    assert!(planes.y.iter().all(|&y| y == 0.5), "luma must be untouched");
    for r in [0, H - 1] {
        let row = &planes.i[r * W..(r + 1) * W];
        assert!(row[..20].iter().all(|&v| v == 0.0), "{row:?}");
        assert!(row[20] > 0.0 && row[20] < 0.4, "{}", row[20]);
        assert!(row[20..].windows(2).all(|p| p[1] < p[0] && p[1] > 0.0));
        let q = &planes.q[r * W..(r + 1) * W];
        assert!((q[20] / row[20] + 0.5).abs() < 1e-5, "Q bleeds like I");
    }
}

#[test]
fn presets_round_trip_and_validate() {
    let mut e = BendEffect {
        seed: 42,
        ..BendEffect::default()
    };
    e.sync.enabled = true;
    e.sync.settings.h_skew = -0.25;
    e.invert.enabled = true;
    e.invert.settings.mode = InvertMode::SwapIq;
    e.address_b.settings.bit = 9;
    assert_eq!(BendEffect::from_json(&e.to_json()).unwrap(), e);

    assert!(matches!(
        BendEffect::from_json(r#"{"version":99}"#),
        Err(PresetError::UnsupportedVersion(99))
    ));

    let c = BendEffect::from_json(r#"{"sync":{"enabled":true,"h_jitter":5.0}}"#).unwrap();
    assert!(c.sync.enabled);
    assert_eq!(c.sync.settings.h_jitter, 0.2);

    let mut d = BendEffect::default();
    d.set_by_id("data.target", "both").unwrap();
    assert_eq!(d.data.settings.target, DataTarget::Both);
    assert!(matches!(
        d.set_by_id("nope.x", "1"),
        Err(PresetError::UnknownSetting(_))
    ));
}
