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
Bevy 0.19.1 is a Cargo dependency, with the dependency tree pinned in
`Cargo.lock`. No separate visual editor or shader compiler is required.

Use `cargo run`, `cargo fmt`, and `cargo clippy` in this environment.
Check GPU availability with `vulkaninfo --summary`.

Commit `flake.lock` alongside `flake.nix` to pin tools and system libraries.
Update them intentionally with `nix flake update`. If using Git, newly created
flake files must be tracked for Nix to see them.

## Launch the editor or game

Both modes use the same executable and workspace source files:

```sh
nix develop --command cargo run --locked
nix develop --command cargo run --locked -- --mode editor
```

With no arguments, `cargo run` launches game mode using `test_workspace`, just
like `cargo run -- --mode game --workspace test_workspace`. Override `--mode`
and/or `--workspace DIR` to select another mode or directory. Game mode recursively
loads validated workspace resources and spawns the first character in sorted file
path order in the first saved world (or on a flat plane for character-only workspaces). The camera fits the
body again when the window resizes. Game mode reads the workspace without
saving changes. Missing directories, missing characters, invalid references,
and empty character bodies report an error and exit with code 1.

`test_workspace/stickman.character` references `stickman.body`. Both are version 1
JSON source documents in the editor's existing format. The body has ten parts:
head, torso, two upper/lower arms, and two upper/lower legs, with hierarchical
elbows and knees and no hands. Open the same directory in editor mode to edit it.
Its rest-pose surface height is 1.565 units (half the original 3.13), with all
bone radii preserved. The compact armature places shoulders beside the chest,
hips beneath it, and knees in front of the hip-to-foot line when bent.
The shorter character wears a
seed-speckled **Strawberry beret** and a winged **Bumblebee backpack**, stored as
editable `.prop` files in `test_workspace/props/`. The character document binds
their anchors to the head's **Crown** mount and torso's **Back** mount. The same
recursive, validated workspace loader resolves these stable IDs in both modes;
no special sample-only spawning code is needed. `library.json` mirrors the assets
for legacy loading.
A legacy workspace containing only `library.json` also loads; its first character
uses array order. The sample also includes 18 voxel tiles and a generated neighbourhood with socket-connected roads, houses, entrance paths, and trees.

Startup is split into launch configuration, an editor plugin, and a game plugin.
Both modes share document loading and body/attachment rendering. Game placement
translates the SDF primitives and prop transforms together and grounds the body
using its animated surface bounds.
The mounted-prop ECS system follows each bone's animated position and rotation,
including character movement and turning, and removes props when their character
is removed. This logical parenting avoids inheriting the SDF bounding proxy's
changing scale.

## V1 creators

The implementation plan and acceptance criteria are in [v1-plan.md](v1-plan.md).
The game starts at **1280×720 physical pixels**, including on HiDPI desktops, and
is resizable at runtime. UI scale and the 3D preview viewport follow the live
client area. Keep the baseline usable, then verify other sizes with
`scripts/resize_smoke.py`.

```sh
export CARGO_HOME="$PWD/.cargo-cache"
cargo run --locked -- --mode editor
```

The editor opens `test_workspace` by default. Use `--workspace DIR` to open
another directory, or `--data-dir DIR` to start at the workspace chooser.
The left Explorer lists its
`.body`, `.prop`, and `.character` source resources; right-click the Explorer
(or use **New ▾**) to create one. Selecting a resource opens its editor in a
tab, so several resources can stay open while you switch among them. The
resource-specific Inspector floats on the right. **Save** writes editable
source files directly in the workspace plus `library.json` as a compatibility
snapshot, with the previous snapshot retained as `library.json.bak`.
Unsaved changes mark the Save button with `*`. **Reopen** reloads the saved
library; Undo can recover the working state from before reopening.

### Create a body

1. Open **Bodies**, choose **New**, then **Rename**. Name entry supports typing,
   Backspace, Ctrl+A to clear, Enter to confirm, and Escape to cancel.
2. **+ Root** starts an empty armature. **+ Child** connects a new bone to the
   selected bone's tip. Use `<` and `>` to select bones.
3. **Tip XYZ** edits the bone's end relative to its joint. **Joint XYZ** edits
   its start relative to the parent's tip (or the origin for a root). Values
   use body axes and 0.1-unit increments. Moving a parent tip moves descendants.
4. Adjust thickness, switch **Capsule / Sphere**, and choose a color. Spheres
   are centered at bone tips. The smooth body is raymarched from these shapes,
   with blended joints and colors; it is not a mesh approximation.
5. Select the head bone, open **Mounts**, and **Add mount**. Name the mount and
   adjust position relative to that bone's tip and rotation in 15-degree steps.
   A newly added mount starts one bone radius above the tip.

**Delete branch** removes the bone, descendants, and their mounts; affected
character attachments are removed too. Undo restores the complete edit.
An optional **New starter body** provides a ready-made six-bone rest pose.
Bodies support up to 32 bones. Animations belong to characters using that body.

### Make a voxel hat or other prop

1. Open **Props**, choose **New**, and name it. The workspace is a 32×32×32 grid;
   one voxel is 0.1 world units.
2. Select a color and a 1×, 2×, 4×, 8×, or 16× brush. The X/Y/Z controls move
   the cursor by the brush size. Changing brush size preserves its position,
   clamped to the grid bounds, so coarse and fine placement can be combined.
3. **Paint cursor** fills the cursor cube; **Erase cursor** removes it. You can
   also click the grid or voxel faces using the selected **Paint / Erase** mode.
   Y sets the plane used when clicking empty space.
4. For a hat, build a wide flat brim and a narrower raised crown. Switching
   colors paints different regions. Large homogeneous blocks remain compact
   in storage; the mesher merges exterior faces and removes internal faces.
5. Position the cursor at the hat's attachment point, switch to **Anchors**,
   and **Add anchor**. Name it and fine-tune its position/rotation. The anchor
   will align to the body mount when assembled.

Ctrl+Z / **Undo** and Ctrl+Y / **Redo** apply to document edits. Fine erasing
inside a coarse block preserves all surrounding voxels.

### Assemble a character

1. Open **Characters**, choose **New**, and name it.
2. The **Body** and **Prop** controls cycle through the available assets. Select
   a body mount and prop anchor with their arrow controls.
3. **Attach prop** aligns the chosen anchor to the chosen mount. Add more props
   to equip multiple items. Item arrows select an attachment; **Remove** removes
   it. Position, rotation, and scale adjust its fit.
4. Save, exit, and reopen: bones, voxels, anchors, and attachments stay editable.

Changing to a different body clears the character's old attachments and animation
tracks because their mounts and bones belong to the previous body. Undo restores the original assembly.
Changes to an existing body or prop are reflected in characters that reference it.

Use the preview's **Orbit**, **Near**, **Far**, and **Guides** controls, or
right-drag to orbit and the wheel to zoom. Guides show bone endpoints, mounts,
anchors, the grid, and the voxel brush. Turn guides off for a clean preview.

## Animate a character

Open a character and choose **Animations** in the Inspector. **New**, **Copy**,
**Rename**, and **Delete** manage any number of named clips. Select a bone in
the left Explorer, move the time cursor with Start/End, ±0.1s, or Key arrows,
then adjust rotation or offset to insert/update a key. **Set key** records the
current pose; **Delete key** removes the key at that time. Length rescales key
times. Play/Pause and Loop control preview playback. Choose **Blend with >**
and a percentage to preview two clips at a shared normalized time. Undo/Redo,
Save, and Reopen work with animation edits.

**Add locomotion** creates missing `standing`, `idle`, `walking`, and `running`
clips for humanoid bones named Torso, Head, Left/Right upper/lower arm/leg.
The sample already contains all four. Standing has subtle breathing; idle adds
sway and head movement. Walking/running are baked from contact, compression,
passing, and recovery poses, with forward knee bends, backward heel recovery,
opposing arm swings, relaxed elbows and a level head. Running has a flight phase.
These are ordinary editable keyframes, not runtime procedural animation.

Generated clips include optional `stride_distance` metadata (zero for standing
and idle). Their root keys are authored relative to the rest-pose ground plane;
playback preserves that vertical motion and advances stride phase by distance
travelled. For these gaits, movement speed scales with body height, so the short
sample walks at 1.3 units/s and runs at 3.25 instead of sliding at the old tall
body's speed. Legacy clips without this metadata keep their existing playback.
After changing proportions, regenerate the four starter clips explicitly with
`cargo run --locked --example animate_workspace -- WORKSPACE --replace-locomotion`.
Other named clips and equipped props are preserved.

Each `.character` has an optional `animations` array. Clips store a unique name,
duration in seconds, loop flag, and sparse tracks keyed by body bone ID. Keys
store time, relative Euler XYZ rotation in radians, and joint translation.
Playback interpolates rotations as quaternions and carries parent motion down
the hierarchy. Missing tracks use the rest pose; old files without animations
remain loadable. Attached props follow the animated mount. Deleting bones also
removes their animation tracks; Undo restores them.

## Play controls

| Action | Keyboard / mouse | Gamepad |
| --- | --- | --- |
| Move | WASD | Left stick (analog speed) |
| Run | Hold Shift | Full stick; B / east button forces sprint |
| Look | Mouse | Right stick |
| Jump | Space | A / south button |
| Recenter behind character | R | Right-stick click |
| Camera distance | Mouse wheel | — |
| Release / resume controls | Esc / click | Start |

Movement is relative to the camera with acceleration, braking, and smooth facing.
The follow camera orbits, clamps pitch, and stays above the ground. Standing
transitions to idle after a few seconds; movement blends standing/walk/run by
speed, with walk/run sharing a stride phase. Missing animation clips fall back
to available poses. Focus loss releases controls. Saved worlds use piece collision volumes, curb stepping, wall/ceiling collision,
and camera obstruction checks. Character-only workspaces retain the flat-ground
playground. Jump-specific animations remain future work.

To add the same starter animations to another workspace with named humanoid
bones, run `cargo run --locked --example animate_workspace -- WORKSPACE_DIR`.
It saves the workspace and preserves existing clips with those names.

## Tiles, sockets, and worlds

`test_workspace/tiles/` contains **actual editable voxel source assets**: grass,
house plots, straight/corner/T/crossing roads, straight/corner walkways, house
floors, door/window/side walls, roof slopes/gables/ridges, and tree trunks,
branches, and foliage. The saved neighbourhood is in `test_workspace/worlds/`.
Roads are at Y=0; grass, sidewalks, entrance paths, and house floors are at Y=0.2.
Plots reserve topsoil space for the house and entrance path, which fill that space
when assembled. Door openings are clear; the starter kit has no animated doors.

```sh
nix develop --command cargo run --locked -- --mode editor --workspace test_workspace --editor-view tiles
nix develop --command cargo run --locked -- --mode editor --workspace test_workspace --editor-view worlds
nix develop --command cargo run --locked -- --mode game --workspace test_workspace
```

Use the **Tiles** and **Worlds** toolbar buttons or select their source resources
in Explorer. Tiles support new/duplicate/rename, voxel paint/erase at an explicit
voxel cursor, socket name/type/profile/position/facing/requirement editing, a **Rules** tab
for custom many-to-many compatibility pairs, and collision-box
authoring. **Frame all** fits the current asset. Socket guides show facing; the
collision tool shows boxes. Source documents also expose profiles and arbitrary
allowed orientation quaternions. Cursor steps are 0.1 world units.

Worlds support selecting a piece in the viewport or with Piece controls, selecting
one of its sockets, choosing a tile and rotation, and **Place**. A green wireframe
previews a valid placement. The first piece is placed at the origin. Subsequent
pieces must connect to the selected free socket. All coincident sockets are
checked and connected, and occupied volumes cannot intersect. **Remove**, Undo,
Redo, Save, and Reopen work on the graph. Incomplete worlds can be saved as drafts;
**Validate** and **Save & play** require a complete world. Play opens a separate
game window. `--world ID` selects a particular saved world's numeric ID.

World storage is a spatial graph of piece instances and socket connections,
not a tile array. Standard 12.8-unit terrain footprints and quarter-turn rotations
are conventions of this starter kit. Smaller house and tree pieces use the same
system. Local socket positions lie on the voxel lattice; socket frames and allowed
instance orientations use quaternions. Connection rules in `connections.sockets`
are symmetric many-to-many type pairs, with exact profile and frame matching.
Collision boxes remain separate from visual geometry; oriented occupancy checks
also protect non-solid foliage. Meshing is chunked and cached across instances.

**Generate new world** uses the selected seed, dimensions (5–11 cells per side),
house count, and planting percentage. The starter generator builds a connected
street-loop layout, selects compatible rotated terrain candidates from metadata
with bounded backtracking, assembles single-storey houses, and connects each door
to a sidewalk. Failed plot/orientation choices are discarded. Optional planting
sockets grow bounded trunk/branch/foliage assemblies. It reports impossible
requests rather than publishing partial worlds. This is a starter construction
recipe; additional layout/tree/house recipes can use the same graph and checks.

Every completed walkway component must connect exactly one door and one sidewalk,
with no branches, dangling ends, or isolated loops. Unused sidewalk access sockets
are valid and do not create paths. Required house/roof/tree sockets must terminate
in compatible pieces. Explicit road exits are allowed only on world boundaries.
Saved worlds also require a supported character spawn.

To create the kit in a character workspace that has no tiles, or generate an
additional world from an existing kit:

```sh
nix develop --command cargo run --locked --example generate_neighbourhood -- WORKSPACE
# Existing kit: seed, width, depth, houses, planting percentage
nix develop --command cargo run --locked --example generate_neighbourhood -- WORKSPACE 23 7 7 3 45
```

The generator saves versioned `.tile`, `.world`, and `.sockets` JSON documents;
old character-only workspaces still load. Existing resource paths are preserved.
`library.json` remains a compatibility snapshot. Source files are loaded in path
order; stable asset IDs preserve graph references independently of that order.

```sh
nix develop --command python3 scripts/tiles_smoke.py
nix develop --command python3 scripts/resize_smoke.py --editor-view worlds
nix develop --command python3 scripts/resize_smoke.py --editor-view tiles
```

The tile scenario uses real controls to author voxels, sockets and collision,
place rotated road pieces, generate/save/reopen a world, and walk from the road
up the curb, along an entrance path, through a doorway, and into a blocking wall.

## Automated visual loop (Linux)

```sh
nix develop --command python3 scripts/visual_smoke.py
# Same full creator scenario, without rebuilding:
nix develop --command python3 scripts/creator_smoke.py --no-build
```

The harness builds the actual game and runs a private Xvfb window using Mesa
software Vulkan. It creates a six-bone body **from scratch through the UI**,
paints a multicolor voxel hat, exercises coarse/fine erasing and undo/redo,
creates an anchor and mount, equips the hat, saves, and restarts the process.
It also exercises direct viewport painting, attachment rotation, command-line
capture, and the capture-failure exit code.

Tests use isolated data under a unique `artifacts/creator-*/` directory. Evidence
includes JSON reports, interaction logs, editable saved data, renderer PNGs,
independent X server PNGs, and UI/state manifests. Creator baseline images are
1280×720. The harness reads actual button bounds from `--state-file`; it sends
real input and never injects a prebuilt character into the tested library.

After a visual change, run the harness and inspect its report/logs. Open the
relevant PNGs with the agent's image-viewing tool and assess the UI, geometry,
body smoothness, and attachment fit. Pixel checks alone are not visual review.
`AGENTS.md` preserves this workflow for subsequent work.

Rust validation:

```sh
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

Unit tests cover model validation, hierarchy and attachment transforms, sparse
voxel edits versus a dense reference, greedy mesh surfaces, persistence,
cascading deletion, and editor undo/reload behavior. `examples/sdf_check.rs`
is an isolated depth/lighting diagnostic used during renderer development.

## Launch and capture options

```sh
cargo run --locked -- --mode editor --workspace data --editor-view characters \
  --capture artifacts/character.png --capture-frame 120 --exit-after-capture
```

- `--mode editor|game` chooses the entry point (default `game`).
- `--workspace DIR` selects the workspace directory (default `test_workspace`) and skips the chooser.
- `--editor-view bodies|props|characters` opens a creator directly. This replaces
  the old `--workspace bodies|props|characters` shortcut.
- `--data-dir DIR` overrides the default workspace with a fallback directory; an explicit
  `--workspace` takes precedence regardless of argument order. With `--mode editor`
  and no editor view flags, this opens the workspace chooser. Harnesses use isolated directories.
- F12 saves `screenshot-001.png`, etc., under `--output-dir DIR` (default
  `artifacts/manual`). Numbering restarts each process.
- `--capture FILE.png --capture-frame N` requests an automatic renderer capture
  after warmup. `--exit-after-capture` exits only after writing it; write errors
  return exit code 1. The harness verifies actual rendered content too.
- `--state-file FILE.json` enables the test manifest; normal play does not write
  this debug stream. It contains working documents, selections, notices, and
  control positions, not commands for modifying the game.

The unattended test uses software rendering because Xvfb lacks the Radeon
DRI3 presentation support required here. Ordinary desktop rendering supports
Vulkan/Wayland; synthetic desktop clicks and X server grabs were unreliable
under this machine's rootless XWayland. Use the private display for automation.
If a sandbox blocks dependencies, Nix, or display sockets, run the same command
through its approved execution mechanism.

Runtime resize coverage:

```sh
nix develop --command python3 scripts/resize_smoke.py
```

This keeps one live window open while resizing it through 1280×720, 1600×900,
800×600, 640×480, and back to 1280×720. It verifies window geometry, control bounds, and
capture dimensions at every size.

To verify the game session and its runtime camera framing, using an
isolated copy of `test_workspace` and real F12 input:

```sh
nix develop --command python3 scripts/resize_smoke.py --mode game
# Verify default mode and workspace using an isolated test_workspace copy:
nix develop --command python3 scripts/resize_smoke.py --mode game --default-launch
# Open the same sample in the character editor while resizing:
nix develop --command python3 scripts/resize_smoke.py --editor-view characters
```

This also checks launch errors, visible body pixels, renderer logs, and that
playing leaves the source files unchanged. Build with `cargo build --locked`
before running either resize scenario.

Animation and gameplay interaction coverage (build first):

```sh
nix develop --command python3 scripts/motion_smoke.py
```

This edits, blends, plays, saves, and reopens animations with real input, checks
animation controls at smaller window sizes, then exercises idle/walk/run,
jumping/landing, mouse orbit/zoom/recenter, and cursor release/recapture.
Gamepad mapping and analog blending also have synthetic Bevy input tests;
these do not substitute for testing a physical controller.
