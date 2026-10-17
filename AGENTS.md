# Development and verification

Follow `spec.md`: Rust, Bevy, source-defined scenes, and decoupled ECS systems.
The editor (`src/editor/`) is a workspace editor in the style of a game-engine
editor: Explorer/Outline on the left, tabbed 3D viewport, Inspector on the right.
Editor state and actions live in `src/editor/{mod,actions,fields}.rs`; the UI in
`src/editor/ui/` is rebuilt from that state. See `README.md` for its features. Character animations and a third-person
playground, socket-connected voxel tiles, and seeded neighbourhood generation are implemented.
The starter tile kit is drawn in `src/kit/` and assembled by `src/generation.rs`;
regenerate `test_workspace` with `examples/generate_neighbourhood.rs --replace-kit`
and review tiles/worlds from an RTS-style camera with `examples/world_shot.rs`.
Run `scripts/tiles_smoke.py` for tile/world editing and terrain collision coverage.
Run `scripts/motion_smoke.py`
for animation editing and keyboard/mouse gameplay interaction coverage.
See `README.md` for `--mode editor|game --workspace DIR`.

For changes affecting rendering or interaction:

- Use **1280×720 physical pixels** as the baseline for creator screenshots,
  but also run `scripts/resize_smoke.py` for runtime sizes. The game must remain
  usable while resizing; do not render a large screenshot and resize it afterward.
  The game overrides HiDPI scaling to 1 and derives UI scale and preview bounds
  from the live client area.
- Run `cargo fmt --check`, `cargo test --locked`, and
  `cargo clippy --locked --all-targets -- -D warnings` in the Nix environment.
- Run `nix develop --command python3 scripts/visual_smoke.py` to build and exercise
  the actual game using a virtual display and real mouse input.
- Run `nix develop --command python3 scripts/resize_smoke.py` after layout or
  window changes. It resizes one live window through 1280×720, 1600×900,
  800×600, 640×480, and back to 1280×720, checking controls and screenshot dimensions.
- Read the resulting `report.json` and game logs. Open the relevant saved PNGs
  with an image-viewing tool and assess them visually; passing pixel checks alone
  does not constitute visual review.
- Extend the scenario or add a command-line entry point when a new feature needs
  different setup. Preserve ordinary interactive navigation.
- The creator harness reads control bounds from `--state-file` and sends real
  input. Keep this manifest accurate; do not replace interaction coverage with
  injected model data. Test data stays isolated with `--data-dir`.
- In sandboxes without Unix sockets, run the scripts with `XVFB_TCP=1` inside a
  mount namespace that gives Xvfb a private `/tmp` (for example
  `unshare -rm sh -c 'mount -t tmpfs none /tmp; exec python3 scripts/...'`).
- Keep generated evidence under ignored `artifacts/`. Report what was actually
  launched and viewed, and distinguish software rendering from hardware testing.

See `README.md` for capture flags and harness options. Restricted sessions can
use `CARGO_HOME="$PWD/.cargo-cache"` to keep dependency-cache writes local.
