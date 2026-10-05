{
  description = "Rust and Bevy game development environment";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

  outputs =
    { nixpkgs, ... }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSystem = nixpkgs.lib.genAttrs systems;
    in
    {
      devShells = forEachSystem (
        system:
        let
          pkgs = nixpkgs.legacyPackages.${system};
          libraries = with pkgs; [
            alsa-lib
            udev
            vulkan-loader
            libx11
            libxcursor
            libxi
            libxrandr
            libxkbcommon
            wayland
          ];
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              rustc
              cargo
              rustfmt
              clippy
              rust-analyzer
              pkg-config
              clang
              lld
              cmake
              gdb
              vulkan-tools
              xorg-server
              xauth
              xdotool
              (python3.withPackages (ps: [ ps.pillow ]))
            ];

            buildInputs = libraries;
            RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";

            shellHook = ''
              # winit and wgpu load some libraries at runtime.
              export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath (libraries ++ [ pkgs.mesa ])}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

              # Supply matching Mesa Vulkan drivers for AMD/Intel on non-NixOS
              # hosts too. Keep the host's NixOS/proprietary drivers preferred.
              export XDG_DATA_DIRS="''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}:${pkgs.mesa}/share"
            '';
          };
        }
      );

      formatter = forEachSystem (system: nixpkgs.legacyPackages.${system}.nixfmt);
    };
}
