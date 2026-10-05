# Body editor UX review passes

1. Replaced the scattered body controls with a clear outline/inspector workflow.
2. Made the bone hierarchy selectable instead of requiring next/previous cycling.
3. Added parent indentation and a durable selected-row treatment.
4. Kept the selected bone highlighted in the 3D armature.
5. Grouped create, delete, and rename commands beside the outline.
6. Replaced long wrapped coordinate labels with Tip/Joint tabs and compact steppers.
7. Grouped shape, radius, and color into the inspector rather than mixing them with tree actions.
8. Added independent wheel scrolling for the outline and inspector panes.
9. Reduced visual noise: short labels, compact swatches, and no redundant status strip while editing bones.
10. Moved outline and properties into one persistent left sidebar so the 3D preview remains the central focus.

The acceptance check is the real-window creator scenario in `scripts/creator_smoke.py`.
