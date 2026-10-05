# Second professional UX review: resource flow and camera

Each pass used the critical question: would a person who knows ordinary file
managers and editors understand the next action without learning an app-specific
trick?

1. **Entry flow** — Category navigation opened the last asset invisibly. Fixed: Bodies, Props, and Characters now open a named resource browser.
2. **Direct selection** — Previous/next was the primary asset selector. Fixed: every resource has a visible, directly clickable row; `New body`, `New prop`, and `New character` are separate actions.
3. **Return path** — An editor had no clear way back to its category. Fixed: the body editor has `All bodies`; category navigation always reopens the browser.
4. **Home composition** — The fixed-offset home group was stranded in the upper left. Fixed: it is centered in the live window below the global navigation.
5. **Mount context** — Switching to Mounts discarded the skeleton workspace. Fixed: Skeleton and Mounts are selected tabs in the same persistent body workspace.
6. **Mount creation** — The old UI hid which bone received a mount. Fixed: the mount view starts with `BONE FOR NEW MOUNTS`; its button explicitly says it adds to the selected bone.
7. **Mount inspection** — A mount was a number in a carousel. Fixed: a named mount list, selected state, parent bone, offset/rotation tabs, and editable coordinates remain visible together.
8. **Orbit** — Camera drag changed yaw only. Fixed: right drag now orbits horizontally and vertically with a safe pitch clamp.
9. **Pan and discoverability** — Camera had no panning. Fixed: middle drag pans relative to the view and the persistent viewport legend documents orbit, pan, and zoom.
10. **Verification and framing** — The changed workflows were captured from the actual renderer at 1280×720. Home, Props browser, and Mounts screenshots were inspected; Frame all remains visible in each editor.

Acceptance evidence:

- [Centered home](/tmp/pro-home-final.png)
- [Props resource browser](/tmp/pro-prop-browser-final.png)
- [Persistent Mounts workspace](/tmp/pro-mounts-final.png)
