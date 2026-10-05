# V1: bodies, voxel props, and assembled characters

## Outcome and limits

From the startup menu, a player can create and save a body from an empty
armature, sculpt a colored voxel prop such as a hat, define its attachment
anchor, and assemble a named character by attaching that prop to a body mount.
Reopening the application restores the editable source data and assembly.
Creator editing and the default visual scenario use **1280×720 physical pixels**;
the resizable window must also reflow correctly at runtime sizes.

Include hierarchical bones, editable rest-pose geometry, per-bone sphere/capsule
shape, thickness and color, smooth raymarched SDF body generation, named mounts,
voxel painting/erasing with coarse brushes, prop anchors, and character assembly.
**No animation**: no keyframes, timeline, animation playback, or locomotion.
Tiles, world generation, and play mode are outside this creator milestone.
The preview uses cartoon lighting and SDF silhouette outlines; the full scene
object-ID outline pipeline remains a separate rendering milestone.

## Architecture

- `model.rs`: versioned, serializable bodies, bones, mounts, sparse voxel props,
  anchors, and character attachments. Stable IDs connect documents. Validate
  ranges, references, hierarchy, and finite numeric values before loading/saving.
- `storage.rs`: JSON library save/load, atomic replacement and backup. Failed
  loads must preserve the current working library. Tests use isolated files.
- `body_render.rs` and WGSL: bounded proxy volume, genuine sphere/capsule SDF
  raymarching with smooth unions and per-bone colors. Write the actual hit depth
  so solid voxel hats correctly intersect/occlude the smooth body.
- `voxel.rs`: sparse power-of-two blocks; split/merge storage on fine/coarse edits;
  greedy surface meshing with internal faces removed and equal-color faces
  combined. Ray picking supports direct viewport painting.
- Editor modules: shared working library and undo history, separate UI action
  systems and preview generation. Rebuild previews when edits change documents.
  Keep orbit camera, selection helpers, and screenshot tooling independent.

## Implementation stages

1. **Documents and persistence.** Create the schemas and validation; add
   hierarchy, transform, save/load and malformed-file tests. Store user data
   separately from test artifacts and provide a `--data-dir` launch option.
2. **Body editor.** Library create/select/name actions. Start empty, add root or
   child bones, select/delete branches, edit joint offset and tip in X/Y/Z,
   change thickness, shape and color. Children follow their parent's tip.
   Show the armature over the generated body; allow orbit/zoom. Provide an
   optional starter body without making it necessary for scratch creation.
3. **Mount editor.** Add/select/name/delete mounts on a selected bone. Edit
   position relative to its tip and rotation. Visualize the selected mount.
4. **Prop editor.** Empty 32³ voxel workspace, 0.1 world units per voxel. Show
   a grid and editing cursor; paint/erase with 1/2/4/8/16 brushes and a palette.
   Support both viewport clicks and explicit X/Y/Z cursor controls for precise
   placement. Undo/redo edits. Add/select/name/delete anchors; edit their
   positions and rotations. Keep stored blocks and generated quads compact.
5. **Character editor.** Create/name/select characters, choose a saved body and
   prop, select a body mount and prop anchor, then attach. Support multiple
   attachments, removal, and position/rotation/scale adjustments. Compute prop
   transforms from mount and anchor frames so the anchor lands on the mount.
6. **End-to-end verification.** Improve the real-window harness to create a
   scratch body, edit its armature and mount, paint a hat with mixed brush sizes,
   assign an anchor, assemble and save a character, restart, and verify that
   documents and visible results survive. Exercise erasing and undo/redo as well.

## Editing usability

Use a compact left tool panel and a right 3D preview with readable labels.
Expose current selection, values, and operation feedback. Keep destructive
actions explicit; undo/redo covers document edits. Name entry uses ordinary
keyboard input. A persistent Save control and dirty indicator make persistence
visible. Surface invalid operations as messages rather than crashing.
Empty bodies/props must be valid and have useful on-screen instructions.

## Acceptance checks and iteration

- Rust unit tests cover hierarchy/cascade deletion, anchor transforms,
  validation, JSON persistence, voxel subdivision/merging, and greedy meshing.
- A solid coarse voxel cube is one stored block and six exterior quads;
  fine edits preserve unrelated volume and correct colors.
- The interactive scenario uses real mouse/keyboard events against the shipped
  application, isolated data, state/log assertions, and bounded waits.
- Default creator review images are 1280×720; resize review images use their
  current window size. Review all of them with the AI image viewer for
  legibility, body smoothness, hat placement, clipping, and persistence. Pixel
  counts alone are insufficient.
- Run formatting, Clippy, relevant unit tests, and the visual scenario. Fix
  implementation or harness problems and repeat the affected checks until the
  complete create → equip → save → reopen workflow passes.
- Record the tested result and any concrete limitations here when complete.

## Progress

Implemented and verified on 2026-09-12. All six stages are complete.

- Body editing starts from an empty armature; the optional starter is separate.
  Generated body surfaces are actual raymarched smooth SDFs with correct depth.
- Props support sparse mixed-size edits, direct viewport painting, greedy
  surfaces, editable anchors, and palette colors converted correctly for rendering.
- Character assemblies support multiple attachments, alignment, rotation,
  scale, removal, and editable persistence. Deleting referenced bones/mounts/
  anchors keeps assemblies valid; undo restores the complete operation.
- Empty assets have valid previews without submitting empty GPU meshes.
  Value labels fit at 1280×720. Keyboard shortcuts handle press/release within
  one frame and recover correctly after focus loss.
- The test manifest exposes actual button bounds and working state. The harness
  uses it for real input, validates rendered and displayed images, and checks
  renderer logs after each action. It does not inject fixture model data.

### Final validation

- `cargo fmt --check`: passed.
- `cargo test --locked`: **20 tests passed** (14 library, 6 editor).
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `nix develop --command python3 scripts/visual_smoke.py --no-build`: passed
  against the validated build. **247 real UI actions** created a six-bone body,
  a two-color voxel hat, a mount, an anchor, and an equipped character, then
  saved and reopened them. Direct viewport painting, erase/undo/redo,
  attachment rotation/scale, automatic capture, and capture-failure exit code
  were also checked.
- Six independent window captures matched the renderer captures exactly.
  Default review PNGs are **1280×720**; resize images match their live window
  dimensions, and normal game logs contain no errors or
  panics. The deliberate unsupported-format test exits with code 1.
- AI visual review confirmed readable controls, smooth body geometry, a
  recognizable voxel hat aligned to the head, and the same assembly after
  restarting. Perspective/orthographic renderer diagnostics separately verified
  body occlusion against foreground and background solid objects.

Final evidence is under `artifacts/creator-20260912-134728-2655561/`, especially
`report.json`, `character-hat.png`, `character-reopened.png`, the state manifests,
and saved `data/library.json`. SDF diagnostic evidence is under
`artifacts/sdf-check/`. Generated evidence is intentionally ignored by Git.

The V1 limits remain: 32 bones, a 32³ prop grid, rest-pose editing only, and
no worlds/play mode or full scene object-ID outline pass. Unattended tests use
software Vulkan on a private virtual display; this is not a GPU performance
benchmark. The normal interactive application uses the available desktop GPU.
