{
  description = "bender-rs: circuit-bent video filter";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-darwin"
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          inherit (pkgs) lib stdenv;
          gst = with pkgs.gst_all_1; [
            gstreamer
            gst-plugins-base
            gst-plugins-good
            gst-plugins-bad
            gst-plugins-ugly
            gst-libav
          ];
          linuxRuntime = with pkgs; [
            libxkbcommon
            wayland
            libGL
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
          ];
        in
        {
          default = pkgs.mkShell (
            {
              packages =
                (with pkgs; [
                  cargo
                  rustc
                  clippy
                  rustfmt
                  rust-analyzer
                  pkg-config
                  just
                  # wasm32 linker for the demo site's bender-wasm build
                  lld
                  # static file server for `just site-serve`
                  python3
                ])
                ++ gst
                ++ lib.optionals stdenv.hostPlatform.isDarwin [ pkgs.libiconv ]
                ++ lib.optionals stdenv.hostPlatform.isLinux linuxRuntime;

              shellHook = ''
                export GST_PLUGIN_SYSTEM_PATH_1_0=${lib.makeSearchPathOutput "lib" "lib/gstreamer-1.0" gst}
              '';
            }
            // lib.optionalAttrs stdenv.hostPlatform.isLinux {
              LD_LIBRARY_PATH = lib.makeLibraryPath linuxRuntime;
            }
          );
        }
      );
    };
}
