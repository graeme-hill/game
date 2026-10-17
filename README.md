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
uses array order. The sample also includes a 100-tile voxel kit and a generated
"Neighbourhood" world (see [Tiles, sockets, and worlds](#tiles-sockets-and-worlds)).

Startup is split into launch configuration, an editor plugin, and a game plugin.
Both modes share document loading and body/attachment rendering. Game placement
translates the SDF primitives and prop transforms together and grounds the body
using its animated surface bounds.
The mounted-prop ECS system follows each bone's animated position and rotation,
including character movement and turning, and removes props when their character
is removed. This logical parenting avoids inheriting the SDF bounding proxy's
changing scale.

## Workshop editor

```sh
export CARGO_HOME="$PWD/.cargo-cache"
cargo run --locked -- --mode editor
```

The editor is laid out like a game-engine editor. It works on a **workspace**:
a folder of source resources, much like opening a project folder in VS Code.

- **Top bar**: File menu (open workspace, save, revert, play), the workspace
  folder, Save (● marks unsaved changes), Undo/Redo, a breadcrumb for the
  active document, **New** and **Play**.
- **Explorer** (left, top): every resource grouped by kind (Bodies, Props,
  Characters, Tiles by category, Worlds) plus the workspace's Socket rules.
  Type in the filter box to narrow the list. Click to open, right-click for
  Open / Rename / Duplicate / Delete, `+` on a folder to create one. Deleting is
  refused while something references the resource (a body used by a character,
  a tile placed in a world); Undo restores deletions, and Save removes the file.
- **Outline** (left, bottom): the parts of the open document — the bone
  hierarchy with mounts under their bones, anchors, equipped props, animation
  clips, sockets and collision boxes, or a world's pieces and their sockets.
  Selecting an item shows it in the Inspector and highlights it in 3D. Up/Down
  move through the list, right-click offers actions for the item, and
  double-click renames it (double-clicking an Explorer row renames a resource).
  The Explorer and Outline scroll to show what you open or select.
- **Tabs and viewport** (center): open documents stay in tabs (● marks unsaved
  ones). The viewport has floating tool and view controls and a corner axis
  indicator. Right-drag orbits, middle-drag (or Shift+right-drag) pans, the
  wheel zooms; on bodies, characters and worlds a left-drag also orbits. `F`
  frames, `G` toggles guides, `1`/`3`/`7`/`0` pick front/side/top/perspective
  views, and `\` hides both side panels to focus on the viewport. Whatever a
  click would select highlights under the pointer.
- **Move handles**: a selected bone (tip and joint), mount, anchor, socket or
  equipped prop gets a gizmo in the viewport. Drag an arrow to move along that axis or the
  center to move in the view plane; hold Ctrl to snap to 0.1. Handles edit the
  same values as the Inspector, and each drag is one undo step.
- **Inspector** (right): properties of the selection in collapsible sections.
  Vector fields show colored X/Y/Z; **drag a number horizontally to scrub it**
  (one undo step per drag), click it to type an exact value (Up/Down step it).
  Names edit in place. Dropdowns choose bodies, props, anchors, mounts and
  socket types.
- **Status bar**: the result of the last action, resource counts, save state.
  Undo/Redo name the step they would revert or reapply ("Undo Move bone tip").

Panels resize by dragging their edges (and the Explorer/Outline divider). The
window starts at **1280×720 physical pixels**, including on HiDPI desktops, and
is resizable at runtime; UI scale and the 3D viewport follow the live client
area, and side panels yield space in small windows. Verify other sizes with
`scripts/resize_smoke.py`.

`--workspace DIR` opens a folder directly; `--data-dir DIR` (without
`--workspace`) starts at the welcome screen to choose one. The welcome screen
and the Open Workspace dialog list recently opened workspaces (stored in
`$XDG_CONFIG_HOME/workshop-editor/recent.json`; not recorded when
`--state-file` is used, so test runs leave it alone). **Save** (Ctrl+S)
writes every resource as its own source file plus `library.json` as a
compatibility snapshot (the previous snapshot is kept as `library.json.bak`).
**Revert** reloads the saved files; Undo can recover the working state.
Closing the window or opening another workspace with unsaved changes asks
whether to save them first (Save / Don't save / Cancel). Tabs have a context
menu (close, close others, close all, rename, duplicate) and close on
middle-click. **F1** (or `?`) shows every keyboard shortcut.

| Shortcut | Action |
| --- | --- |
| Ctrl+S / Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) | Save / undo / redo |
| Ctrl+N, Ctrl+D, Ctrl+W, Ctrl+Tab | New menu, duplicate, close tab, next tab |
| Ctrl+O | Open workspace |
| F2 / Del / Esc | Rename / delete / deselect the selection |
| B / E / V, `[` `]` | Paint / erase / select tools, brush size |
| Space, `,` `.`, ← → | Play animation, previous/next key, step 1/30 s |
| ↑ ↓, double-click | Move through the Outline, rename |
| M, Alt+click | Mirror painting, pick a voxel color |
| `\` | Hide/show the side panels |
| K | Key the selected bone at the playhead |
| F5 | Save and play |
| F1, `?` | Keyboard shortcuts |

### Create a body

1. **New → Body**; the new resource's name is selected, so just type and Enter.
2. **Add root bone** starts an armature (or use the **Humanoid template**). With
   a bone selected, **Add child** connects a new bone to its tip. Click bones in
   the Outline or the viewport to select them.
3. **Joint** is the bone's start relative to its parent's tip (or the origin
   for a root); **Tip** is its end relative to the joint. Moving a parent moves
   its descendants.
4. Choose **Capsule / Sphere**, a radius and a color. Spheres are centered at
   bone tips. The smooth body is raymarched from these shapes, with blended
   joints and colors; it is not a mesh approximation.
5. **Add mount** on a selected bone creates an attachment point one radius
   above its tip; give it a name, offset and rotation.
6. **Mirror branch** copies a bone and its children (and their mounts) to the
   other side, swapping Left/Right and L/R in the names. The **Parent**
   dropdown moves a bone under another bone (or makes it a root) while keeping
   its joint where it is.

**Delete branch** removes the bone, its descendants and their mounts; affected
character attachments and animation tracks are removed too, and Undo restores
the complete edit. Bodies support up to 32 bones.

### Make a voxel hat or other prop

1. **New → Prop**. The grid is 32×32×32; one voxel is 0.1 world units.
2. Pick **Paint** or **Erase**, a 1–16 brush and a color (palette or RGB;
   Alt+click a voxel to pick up its color). Click or drag in the viewport:
   clicks paint on the face under the pointer or on the cursor's layer in empty
   space, and a drag paints along the plane it started on. **Mirror across X**
   (`M`) paints both halves symmetrically. The voxel cursor can also be typed
   exactly, with **Paint/Erase at cursor**.
3. Large homogeneous blocks remain compact in storage; the mesher merges
   exterior faces and removes internal faces. Fine erasing inside a coarse block
   preserves the surrounding voxels.
4. With the **Select** tool, clicking moves the voxel cursor; **Add anchor at
   cursor** creates the point that aligns with a body mount when equipped.

### Assemble a character

1. **New → Character** (it uses the first body; change it from the Body
   dropdown). Changing to a different body clears the old attachments and
   animation tracks because their mounts and bones belong to that body; Undo
   restores them.
2. In **Equip a prop**, choose a prop, its anchor and a body mount (clicking a
   mount in the viewport chooses it too), then **Attach**. Select an equipped
   prop in the Outline to adjust its offset, rotation and scale or remove it.
3. Changes to a body or prop are reflected in every character that uses it.

## Animate a character

Clips are listed under **Animations** in a character's Outline; **New clip**
and **Add locomotion** are in the Inspector. Selecting a clip opens the
**timeline** under the viewport: transport controls, loop, the playhead (click
or drag the ruler), key markers for all bones and for the selected bone, and a
**Blend** dropdown that previews two clips at a shared normalized time.

Select a bone (Outline or viewport) to see its **Pose** at the playhead. Editing
a rotation or offset value creates or updates that bone's key; **Key** records
the current pose and the ✕ button deletes the key at the playhead. Drag a key
marker to retime it (on the **All bones** row, the whole pose at that time
moves together); click a marker to jump to it. ← → step the playhead by a
frame (1/30 s). The clip's
Length rescales key times. Undo/Redo, Save and Revert cover animation edits.

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

`test_workspace/tiles/` contains **actual editable voxel source assets**, drawn by
`src/kit/` and grouped by category in the Explorer:

- **Terrain cells** (12.8 × 12.8 units): asphalt roads (straight, crosswalk,
  corner, T, crossing), a cobbled brick **lane** style with its own straight,
  corner and T pieces plus road/lane junctions, a stone road **bridge**, streams
  (straight, bend, and a path crossing that takes a footbridge), gravel
  footpaths (straight, corner, T, lookout end), parks (meadow, wildflower meadow,
  pond with a jetty, fountain plaza, community garden, playground), house and
  shop **lots**, and a **hill**: hill top, cliff, cliff corner, cliff stairs,
  a terraced grass ramp, a brook, a spring and a waterfall.
- **Buildings** stack on lots through sockets: house ground floors and upper
  floors in four colours with gable or hip roofs, and shop blocks (corner shop,
  cafe, florist, apartment lobby) with brick, pink balcony, cream or mint
  storeys and flat or garden roofs. A chapel is a single landmark piece.
- **Nature and dressing**: oak, round, maple, golden, blossom, apple, pine,
  birch, poplar and street trees; bushes; tulips, lavender, sunflowers, flower
  beds, tall grass and mushrooms; boulders, stones, rock clusters and stumps;
  vegetable patches, sheds and a gazebo; picket, wooden, hedge, stone and iron
  fences; street lamps, benches, bins, hydrants, post boxes, planters, cafe
  tables, picnic tables and a bus stop; a wooden footbridge and stepping stones.

Scale follows the 1.565-unit character: one voxel is 0.1 units, lower ground is
at Y=0.2 with roads at Y=0 (a 0.2 curb), streams sit at Y=-0.4 with shallow
wadeable beds, and the hill top is 3.4 units up. House doors are 1.4 × 2.4 and
open into a floor at Y=0.4. Stairs and ramps rise in 0.2 steps the character can
climb. Cells carry `edge-0..3` sockets (−Z, +X, +Z, −X) whose type says what
crosses the edge (`road`, `lane`, `curb`, `ground`, `path`, `stream`,
`frontage`, `high`, `brook`, and the handed `cliff-l`/`cliff-r` for the sides of
a cliff), so neighbouring cells only fit when roads, water, paths and cliffs line
up. Decorations attach to upward `plant`, `street`, `fence` and lot sockets.
Shops, trees and roofs are designed to read from a high RTS-style camera as well
as the third-person view; the directional light casts shadows in game mode and
the editor.

```sh
nix develop --command cargo run --locked -- --mode editor --workspace test_workspace --editor-view tiles
nix develop --command cargo run --locked -- --mode editor --workspace test_workspace --editor-view worlds
nix develop --command cargo run --locked -- --mode game --workspace test_workspace
```

Open tiles and worlds from the Explorer (tiles are grouped by category).
Tiles use the same voxel brush as props, plus **Socket** and **Collision** tools:
clicking near a socket selects it, and elsewhere a click moves the voxel cursor
where **Add socket** / **Add box** create new ones. A socket has a name, a type
(choose from the dropdown or create a new type), a profile, a position in voxel
units, a facing (**Turn 90°**) and a **Required** flag; guides show sockets as
markers with facing arrows (required ones in pink). The ⟳ tool previews each
allowed rotation. Source documents also expose arbitrary allowed orientation
quaternions.

**Socket rules** is its own document: a symmetric matrix of socket types where
clicking a cell allows or forbids that pair. New types can be added there too.

In a world, click a piece to select it (the Outline lists its sockets). Open
sockets glow; **click an open socket to get a menu of every tile and rotation
that fits there**, and choose one to place it. The Inspector offers the same
choice with a tile dropdown, rotation and **Place**, with a green box previewing
a valid placement. An empty world starts by placing a chosen tile at the origin.
All coincident sockets are checked and connected, and occupied volumes cannot
intersect. **Complete piece** fills the selected piece's required sockets — and
those of everything it adds — with fitting tiles (preferring tiles that need
the least, varied by the generator seed): for example, a house ground floor
placed on a lot gets its roof. It is one undo step and changes nothing if a
required socket cannot be filled. **Remove**, Undo, Redo, Save, and Revert work on the graph.
Incomplete worlds can be saved as drafts; **Validate** and **Play world**
require a complete world. Play opens a separate game window. `--world ID`
selects a particular saved world's numeric ID.

World storage is a spatial graph of piece instances and socket connections,
not a tile array. Standard 12.8-unit terrain footprints and quarter-turn rotations
are conventions of this starter kit. Buildings, trees and street furniture use
the same system. Local socket positions lie on the voxel lattice; socket frames and allowed
instance orientations use quaternions. Connection rules in `connections.sockets`
are symmetric many-to-many type pairs, with exact profile and frame matching.
Collision boxes remain separate from visual geometry; oriented occupancy checks
also protect non-solid foliage; a per-tile broad phase keeps placement and
validation fast in worlds with hundreds of pieces. Meshing is chunked and cached
across instances.

**Generate new world** uses the selected seed, dimensions (5–11 cells per side),
number of building lots ("Houses"), and planting percentage. The generator plans
a hill in a seeded corner (spring, brook and waterfall, stairs and a ramp down
its cliffs), a stream running from the waterfall across the map (bridged where
streets cross it, with a footpath and footbridge), a street grid with an
optional brick lane, lots facing the streets (shops prefer the main street), and
parks in the remaining cells. Each cell gets a tile and quarter turn whose edge
sockets match the plan, placed through the ordinary socket and occupancy checks.
Buildings are then stacked on lots, and fences, street furniture, trees, bushes,
flowers and rocks fill decoration sockets, skipping anything that would overlap.
It reports impossible requests (for example more lots than street frontage)
rather than publishing partial worlds.

User-made `walkway` tiles keep their rule: every completed walkway component must
connect exactly one door and one sidewalk, with no branches, dangling ends, or
isolated loops. Required sockets must terminate in compatible pieces. Explicit
road exits are allowed only on world boundaries. Saved worlds also require a
supported character spawn; the generator spawns the character on the road in
front of a house, facing its door.

To create the kit and the sample neighbourhood (9 × 9 cells, 16 lots) in a
workspace that has no tiles, rebuild an existing workspace's kit and worlds, or
generate an additional world from an existing kit:

```sh
nix develop --command cargo run --locked --example generate_neighbourhood -- WORKSPACE
# Replace the workspace's tiles, rules and worlds with the current kit:
nix develop --command cargo run --locked --example generate_neighbourhood -- WORKSPACE --replace-kit
# Existing kit: seed, width, depth, lots, planting percentage
nix develop --command cargo run --locked --example generate_neighbourhood -- WORKSPACE 23 7 7 6 45
```

To review tiles and worlds without the editor UI, render them from an
RTS-style camera (optionally with `--shot NAME:YAW:PITCH:DISTANCE[:X:Y:Z]`):

```sh
nix develop --command cargo run --locked --example world_shot -- test_workspace artifacts/shots --shadows
nix develop --command cargo run --locked --example world_shot -- test_workspace artifacts/tiles --shadows --gallery tree,building
```

The generator saves versioned `.tile`, `.world`, and `.sockets` JSON documents
(arrays of plain values and short flat objects such as voxel volumes are written
on one line); old character-only workspaces still load. Existing resource paths are preserved.
`library.json` remains a compatibility snapshot. Source files are loaded in path
order; stable asset IDs preserve graph references independently of that order.

```sh
nix develop --command python3 scripts/tiles_smoke.py
nix develop --command python3 scripts/resize_smoke.py --editor-view worlds
nix develop --command python3 scripts/resize_smoke.py --editor-view tiles
```

The tile scenario uses real controls to author voxels, sockets and collision,
toggle a rule in the matrix, place a road corner by clicking an open socket in
the viewport, generate/save/revert/reopen a world, and walk from the road up the
curb, across the sidewalk and front garden path, through a house's doorway, and
into its back wall.

## Automated visual loop (Linux)

```sh
nix develop --command python3 scripts/visual_smoke.py
# Same full creator scenario, without rebuilding:
nix develop --command python3 scripts/creator_smoke.py --no-build
```

The harness builds the actual game and runs a private Xvfb window using Mesa
software Vulkan. Starting at the welcome screen, it opens an empty workspace and
creates a six-bone body **from scratch through the UI** (typing exact values and
drag-scrubbing a field, with one undo step per drag), paints a multicolor voxel
hat, exercises coarse/fine erasing and undo/redo, creates an anchor and mount,
equips the hat, saves, and restarts the process. It also exercises direct
viewport painting, attachment rotation, command-line capture, and the
capture-failure exit code.

Tests use isolated data under a unique `artifacts/creator-*/` directory. Evidence
includes JSON reports, interaction logs, editable saved data, renderer PNGs,
independent X server PNGs, and UI/state manifests. Creator baseline images are
1280×720. The harness reads actual control bounds from `--state-file` (and
wheel-scrolls panes to reveal controls, like a user); it sends real input and
never injects a prebuilt character into the tested library.

Where Unix sockets are unavailable (restricted sandboxes), set `XVFB_TCP=1` to
run Xvfb on a localhost TCP display instead; Xvfb still needs a writable `/tmp`
for its lock file. The scripts run the binary from `$CARGO_TARGET_DIR/debug`
when that variable is set.

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
cascading deletion, and editor behavior: tabs, selection, typed and scrubbed
values, drag gestures as single undo steps, resource duplication and deletion,
socket placement options, and undo/revert. `examples/sdf_check.rs`
is an isolated depth/lighting diagnostic used during renderer development.

## Launch and capture options

```sh
cargo run --locked -- --mode editor --workspace data --editor-view characters \
  --capture artifacts/character.png --capture-frame 120 --exit-after-capture
```

- `--mode editor|game` chooses the entry point (default `game`).
- `--workspace DIR` selects the workspace directory (default `test_workspace`) and skips the chooser.
- `--editor-view bodies|props|characters|tiles|worlds` opens the first resource
  of that kind directly.
- `--data-dir DIR` overrides the default workspace with a fallback directory; an explicit
  `--workspace` takes precedence regardless of argument order. With `--mode editor`
  and no editor view flags, this opens the welcome screen. Harnesses use isolated directories.
- F12 saves `screenshot-001.png`, etc., under `--output-dir DIR` (default
  `artifacts/manual`). Numbering restarts each process.
- `--capture FILE.png --capture-frame N` requests an automatic renderer capture
  after warmup. `--exit-after-capture` exits only after writing it; write errors
  return exit code 1. The harness verifies actual rendered content too.
- `--state-file FILE.json` enables the test manifest; normal play does not write
  this debug stream. It contains working documents, selections, notices, every
  visible control's action and bounds, scroll panes, and the on-screen positions
  of open world sockets — not commands for modifying the game.

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
