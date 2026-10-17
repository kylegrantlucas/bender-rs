check:
    cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace

gui *ARGS:
    cargo run --release -p bender-gui -- {{ARGS}}

cli *ARGS:
    cargo run --release -p bender-cli -- {{ARGS}}

fixture:
    ffmpeg -y -f lavfi -i testsrc2=size=1280x720:rate=30 -f lavfi -i sine=frequency=440 -t 3 -c:v libx264 -pix_fmt yuv420p -c:a aac /tmp/bender-in.mp4
    ffmpeg -y -f lavfi -i testsrc2=size=1280x720 -frames:v 1 -update 1 /tmp/bender-in.png

# Build the static demo site into site/dist (effect core compiled to wasm32).
site:
    cargo build --release -p bender-wasm --target wasm32-unknown-unknown
    rm -rf site/dist
    mkdir -p site/dist
    cp -R site/src/. site/dist/
    cp -R presets site/dist/presets
    cp target/wasm32-unknown-unknown/release/bender_wasm.wasm site/dist/bender.wasm

# Build and serve the demo site on loopback.
site-serve PORT="8471": site
    python3 -m http.server {{PORT}} --bind 127.0.0.1 --directory site/dist
