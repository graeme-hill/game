# Game

A Rust/Bevy world-building toy, described in [spec.md](spec.md).

## Development environment

Install Nix with `nix-command` and `flakes` enabled, and direnv with its shell
hook configured. From this directory, run:

```sh
direnv allow
```

The `.envrc` uses direnv's `use flake` integration (provided by recent direnv
versions or nix-direnv). Alternatively, enter the same environment directly:

```sh
nix develop
```

The flake supports x86_64 and aarch64 Linux. It includes Rust, Cargo, rustfmt,
Clippy, rust-analyzer and Rust sources, native compilers/linkers, CMake,
pkg-config, GDB, and Vulkan diagnostics. Bevy's native dependencies cover
X11/Wayland windows, ALSA audio, udev controller input, and Vulkan graphics.
Mesa Vulkan drivers are included for AMD/Intel GPUs, including on Ubuntu.
Proprietary NVIDIA drivers on non-NixOS hosts require a matching nixGL wrapper.

These dependencies follow [Bevy's Linux setup guide](https://github.com/bevyengine/bevy/blob/main/docs/linux_dependencies.md).
Bevy itself will be a Cargo dependency when the game is scaffolded; there is
no Cargo project yet. No separate visual editor or shader compiler is required
for the planned Bevy/WGSL workflow.

Once the Cargo project exists, use `cargo run`, `cargo test`, `cargo fmt`, and
`cargo clippy` in this environment. Check GPU availability with `vulkaninfo --summary`.

Commit `flake.lock` alongside `flake.nix` to pin tools and system libraries.
Update them intentionally with `nix flake update`. If using Git, newly created
flake files must be tracked for Nix to see them.
