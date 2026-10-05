# Professional body-workspace review

This is the second, stricter review cycle for the body editor. Each pass was
made against the 1280×720 desktop workspace and the resizable-window rules.

1. **Composition** — The former 208px rail made labels and controls feel compressed. Widened it to 280px and moved the preview boundary to 300px.
2. **Scale** — The old layout scaled from 640×480 and became oversized on normal displays. It now treats 1280×720 as the design size and only scales down or modestly up.
3. **Typography** — Replaced the embedded mono default with DejaVu Sans for a conventional application face and increased control text to 13.5px.
4. **Color** — Removed the saturated blue chrome. Application surfaces, buttons, selected rows, guides, and background now use neutral charcoal values.
5. **Navigation** — Replaced cramped arrow captions with familiar chevrons and gave asset creation a clear “New body” label.
6. **Hierarchy** — Made the skeleton/mount selector full-width and widened tree rows so names and indentation have room to breathe.
7. **Tree actions** — Expanded “Add root” and “Add child” into explicit, evenly spaced controls; delete and rename now form a balanced full-width pair.
8. **Properties** — Increased the independent properties viewport to 220px, widened tabs and coordinate fields, and made radius/shape actions self-describing.
9. **Selection** — Selected tree rows now use a restrained high-contrast gray state rather than a bright game-blue fill; selected bones retain a near-white scene guide.
10. **Responsive preview** — Camera input and rendering use the same live 300px sidebar boundary and 48px header offset, so no input or preview region is stranded by a resize.

An independent reviewer then checked the screenshot and the running-layout
code. Their two blocking findings were corrected: pointer hit testing used the
old 640×480 preview bounds even after the renderer had been resized, and there
was no way to recover the camera composition. Picking/painting now read the
camera's live viewport, and **Frame all** frames the complete armature bounds.
The follow-up also added a persistent status line, left-aligned tree entries,
descriptive property groups, an explicit `Delete branch` label, and clear
`Bone end` / `Joint offset` language.
