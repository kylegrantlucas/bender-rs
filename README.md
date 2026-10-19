# bender-rs

[![CI](https://github.com/kylegrantlucas/bender-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/kylegrantlucas/bender-rs/actions/workflows/ci.yml)
[![Demo site](https://github.com/kylegrantlucas/bender-rs/actions/workflows/pages.yml/badge.svg)](https://klr.dev/bender-rs/)
[![Latest release](https://img.shields.io/github/v/release/kylegrantlucas/bender-rs?sort=semver)](https://github.com/kylegrantlucas/bender-rs/releases/latest)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Rust 1.96+](https://img.shields.io/badge/rust-1.96%2B-orange?logo=rust)](https://www.rust-lang.org/tools/install)

bender-rs is a video filter that imitates circuit bending, the practice of soldering wires across a camera processor or video mixer until the picture tears, rolls, smears and changes colour. Each bend models one stage of a video path, from the camera sensor through tape, sync, sampling, codec and frame store to the colour encoder, the monitor and a feedback loop, and breaks it the way a stray wire would.

![The demo test card and the original eight bundled presets, animated](docs/presets.webp)

<sub>Top left is the unbent test card. The other tiles are the original eight bundled presets, each rendered from that clip with `bender --preset presets/<name>.json`.</sub>

## Try it in your browser

The [demo site](https://klr.dev/bender-rs/) runs the effect as WebAssembly on a test card, your webcam, or your own image or video. Switch bends on along the signal path, tune them, and copy the matching `bender` command to render a full-length file with sound.

[![The demo site with the meltdown preset applied](docs/screenshot.png)](https://klr.dev/bender-rs/)

## What's included

- `bender`, a command-line renderer for videos and images
- `bender-gui`, a desktop app with a live preview and a render button
- the effect core, a plain Rust library with no media dependencies, which also runs the browser demo

## Install

### Prebuilt binaries

Download the archive for your platform from the [latest release](https://github.com/kylegrantlucas/bender-rs/releases/latest). Builds exist for Linux (x86_64 and arm64) and macOS on Apple silicon. Each archive has `bender`, `bender-gui`, the bundled presets and the licenses.

Both programs use GStreamer to decode and encode media, so install it first.

macOS (Homebrew):

```sh
brew install gstreamer
```

Debian and Ubuntu:

```sh
sudo apt install gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
```

On other distributions, install the GStreamer base, good, bad, ugly and libav plugin sets. If a plugin is missing, `bender` names the element and the package it comes from.

The macOS binaries are not signed. If Gatekeeper refuses to open them, clear the quarantine flag:

```sh
xattr -d com.apple.quarantine bender bender-gui
```

### From source

You need Rust 1.96 or newer and the GStreamer development headers (`libgstreamer1.0-dev` and `libgstreamer-plugins-base1.0-dev` on Debian and Ubuntu; Homebrew's `gstreamer` already includes them).

```sh
cargo install --locked --git https://github.com/kylegrantlucas/bender-rs bender-cli
cargo install --locked --git https://github.com/kylegrantlucas/bender-rs bender-gui
```

With Nix, `nix develop` gives you a shell with the toolchain, GStreamer and every plugin.

## Usage

### Command line

Render a video with one of the bundled presets:

```sh
bender --preset presets/meltdown.json -i input.mp4 -o bent.mp4
```

The output extension picks the format: `.mp4` is H.264 with AAC audio, `.mkv` is lossless FFV1 with FLAC audio, and `.png` renders a still. Images work as input too.

Change individual settings with `--set`, which you can repeat. It applies on top of a preset, or on top of the defaults if you leave the preset out:

```sh
bender -i input.mp4 -o bent.mp4 \
  --set sync.enabled=true --set sync.v_roll=0.3 \
  --set burst.enabled=true --set burst.phase=90
```

Other useful flags:

| Flag | What it does |
| --- | --- |
| `--list-settings` | Print every setting with its range, default and a description |
| `--dump-preset` | Print the combined preset as JSON instead of rendering |
| `--seed N` | Pick a different random pattern for the same settings |
| `--scanlines N` | Process at N lines of height, 480 by default; 0 keeps the source resolution |
| `--quantizer N` | H.264 quality from 0 to 51, lower is better; 18 by default |
| `--codec h264\|ffv1\|png` | Override the format chosen from the extension |
| `--overwrite` | Replace an existing output file |

Run `bender --help` for the full list.

Settings measured in pixels and rows, such as sample hold, address bits and band heights, apply at the processing size. The same preset looks coarser at `--scanlines 240` and finer at `--scanlines 0`.

### Desktop app

```sh
bender-gui            # then use "Open…"
bender-gui clip.mp4   # or open a file directly
```

The app plays the source through the effect while you adjust it, with sound. It loads and saves the same JSON presets as the CLI, and its render panel writes the same formats.

## The bends

By default the effect runs these stages in this order, grouped here by the part of the hardware they model. Each is off until you enable it. A preset's `order` list, the arrows in the desktop app and the move buttons on the demo site rearrange them, so you can, for example, colourize the feedback loop's output instead of its input, or bend the frame store after the colour encoder.

```mermaid
flowchart TB
  in([Video in]) --> Cam
  subgraph Cam[Camera]
    direction LR
    sensor[sensor] --> retina[retina] --> isp[isp]
  end
  subgraph Tape[Tape]
    direction LR
    tape[tape]
  end
  subgraph TBC[TBC / sync]
    direction LR
    sync[sync]
  end
  subgraph ADC[ADC]
    direction LR
    clock[clock] --> data[data]
  end
  subgraph Codec[Codec]
    direction LR
    dct[dct] --> mosh[mosh]
  end
  subgraph RAM[Frame store]
    direction LR
    hold[hold] --> counter[counter] --> address_a[address_a] --> address_b[address_b]
  end
  subgraph Amp[Proc amp / DAC]
    direction LR
    colorize[colorize] --> invert[invert] --> rail[rail] --> osc[osc]
  end
  subgraph Enc[Colour encoder]
    direction LR
    burst[burst] --> bleed[bleed]
  end
  subgraph Mon[Monitor]
    direction LR
    composite[composite] --> scan[scan]
  end
  subgraph Loop[Loop]
    direction LR
    feedback[feedback]
  end
  Cam --> Tape --> TBC --> ADC --> Codec --> RAM --> Amp --> Enc --> Mon --> Loop --> out([Video out])
```

| ID | Circuit | What goes wrong |
| --- | --- | --- |
| `sensor` | Image sensor | Rolling-shutter skew and slip, vertical smear and blooming from highlights, a mis-phased colour filter and fixed-pattern noise |
| `retina` | Retina camera | A Game Boy Camera sensor: a tiny grid, edge enhancement and four dithered shades |
| `isp` | Camera ISP | Auto-exposure pumps, white balance hunts, sharpening halos edges and gain noise rises |
| `tape` | Tape transport | Head-switching skew, tracking noise, dropouts, soft luma and smeared colour-under chroma |
| `sync` | Sync separator | Lines lose horizontal lock and drift, tears relock as they go down, the blanking bar shows and the picture rolls |
| `clock` | Pixel clock | Samples are held across pixels, the sample rate stretches, the clock jitters and stalls |
| `data` | Data bus | Bits of each 8-bit sample are swapped, wired together, stuck or latched late |
| `dct` | DCT codec | 8×8 blocks with a bent quantizer table, scrambled coefficients, DC errors and misplaced macroblocks |
| `mosh` | Datamosh | Keyframes are lost and the live motion drags the stale picture around |
| `hold` | Memory hold | Bands, fields or whole frames keep the previous picture |
| `counter` | Address counters | A wrong line pitch shears the picture, early counter resets tile it and the start address scrolls |
| `address_a`, `address_b` | Address bridges | Frame-store address bits are forced or combined, which shuffles blocks of the image |
| `colorize` | Colourizer | Luma is folded, sliced into bands and painted with hues behind a luma key |
| `invert` | Inversion | Luma or chroma inverts, I and Q swap, or the image solarizes |
| `rail` | Rail sag | Bright lines dim and shrink, highlights streak, mains hum rolls and bends lines, ripple draws bars |
| `osc` | Oscillation | A self-oscillating op-amp draws drifting diagonal bars and rings after edges |
| `burst` | Colour burst | Hue shifts and wobbles, alternate lines flip or lose colour, saturation swings and colour drops out |
| `bleed` | Chroma bleed | Colour smears to the right of edges and slips out of register with luma |
| `composite` | Composite link | A real NTSC signal through a cheap decoder: dot crawl, rainbows, ghosts and a detuned subcarrier |
| `scan` | Scan processor | Scanlines lift with brightness into a relief of lines and a wobble bends the raster |
| `feedback` | Feedback loop | The last output returns zoomed, rotated and hue-shifted, like a camera filming its own monitor |

`bender --list-settings` documents every parameter.

## Presets

A preset is a JSON file with a seed and one block per bend. Blocks you leave out stay disabled, and out-of-range values are clamped. An optional `order` lists stage IDs to run first; the rest follow in the default order.

```json
{
  "version": 2,
  "seed": 3,
  "sync": { "enabled": true, "h_skew": 0.0, "h_jitter": 0.035, "tear_rate": 0.45, "v_roll": 0.35 },
  "rail": { "enabled": true, "sag": 0.35, "streak": 0.4, "hum": 0.5 }
}
```

That is `presets/rolling-tear.json`. The directory has other starting points: `address-shuffle`, `bit-rot`, `burst-drift`, `chroma-bleed`, `datamosh`, `dot-crawl`, `feedback-tunnel`, `frozen-store`, `meltdown`, `op-amp-howl`, `pocket-camera`, `scan-lines`, `sensor-bloom`, `stride-shear` and `worn-tape`. The demo site, the desktop app and `--dump-preset` all write this format. On the command line, `--set order=feedback,colorize` reorders the chain.

Version 1 presets still load, but `sync` and `rail` gained settings whose defaults change how those presets look: correlated jitter (`jitter_smooth`), a visible blanking bar (`blanking`) and hum-bent lines (`hum_bend`). Set `jitter_smooth` and `hum_bend` to 0 and `blanking` to false to get closer to the old look.

## Using the library

The `bender` crate has no media dependencies. Give it RGBA8 or YIQ frames with a frame number, and keep one `BendState` per stream: it holds the memory of the stages that look back in time (the frame store, the sensor, the ISP loops, the datamosh references and the feedback loop). For GStreamer pipelines written in Rust, `bender-gst` provides the effect as an element, `BenderFilterElement`.

| Crate | Contents |
| --- | --- |
| `crates/bender` | Effect core: settings, presets and the processing stages |
| `crates/bender-gst` | GStreamer filter element, decoding, preview and render pipelines |
| `crates/bender-cli` | The `bender` command |
| `crates/bender-gui` | The `bender-gui` desktop app (egui) |
| `crates/bender-wasm` | WebAssembly exports used by the demo site |

## Development

`nix develop` sets up everything. Without Nix, install Rust, GStreamer with its development headers, `just`, and the `wasm32-unknown-unknown` target if you work on the site.

```sh
just check        # rustfmt, clippy and the test suite
just cli -i in.mp4 -o out.mp4 --preset presets/bit-rot.json
just gui
just fixture      # write test inputs to /tmp/bender-in.mp4 and /tmp/bender-in.png (needs ffmpeg)
just site-serve   # build the demo into site/dist and serve it on http://127.0.0.1:8471
```

The demo site is static HTML and JavaScript around the `bender-wasm` module. Pushes to `main` deploy it to GitHub Pages.

### Releasing

Bump `version` in the root `Cargo.toml`, commit, then push a matching tag:

```sh
git tag v0.2.0
git push origin v0.2.0
```

The release workflow builds the Linux and macOS archives, checks that the tag matches the crate version, and publishes a GitHub release with checksums. To test the builds without publishing, start the Release workflow by hand from the Actions tab; the archives are attached to that run as artifacts.

## License

Licensed under either the [MIT license](LICENSE-MIT) or the [Apache License, Version 2.0](LICENSE-APACHE), at your option.
