//! Read-only verification manifest for the real-input harness: editor state
//! plus every visible control's action and on-screen bounds.
use super::*;

#[allow(clippy::too_many_arguments)]
pub fn snapshot(
    e: Res<Editor>,
    windows: Query<&Window>,
    options: Res<LaunchOptions>,
    rect: Res<viewport::ViewportRect>,
    cameras: Query<(&Camera, &GlobalTransform), With<viewport::PreviewCamera>>,
    panes: Query<(
        &ScrollArea,
        &ComputedNode,
        &UiGlobalTransform,
        &ScrollPosition,
    )>,
    controls: Query<(
        &Control,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&CalculatedClip>,
    )>,
    mut ticks: Local<u32>,
) {
    let Some(path) = &options.state_file else {
        return;
    };
    *ticks += 1;
    if !(*ticks).is_multiple_of(5) {
        return;
    }
    let Some(window) = windows.iter().next() else {
        return;
    };
    let (width, height) = (
        window.resolution.physical_width() as f32,
        window.resolution.physical_height() as f32,
    );
    let screen = Rect::from_corners(Vec2::ZERO, Vec2::new(width, height));
    let controls: Vec<_> = controls
        .iter()
        .filter(|(.., visible, _)| visible.get())
        .filter_map(|(c, node, t, _, clip)| {
            let mut r = Rect::from_center_size(t.translation, node.size());
            if let Some(clip) = clip {
                r = r.intersect(clip.clip);
            }
            r = r.intersect(screen);
            let (center, size) = (r.center(), r.size());
            (size.x > 0. && size.y > 0.).then(|| {
                serde_json::json!({
                    "label": c.label,
                    "action": format!("{:?}", c.action),
                    "kind": format!("{:?}", c.kind),
                    "center": [center.x, center.y],
                    "size": [size.x, size.y],
                })
            })
        })
        .collect();
    // Open world sockets are 3D click targets; publish where they appear.
    let mut sockets = vec![];
    if let (Some(w), Some(r), Ok((camera, transform))) =
        (e.active_world(), rect.rect, cameras.single())
    {
        for i in &w.instances {
            let Ok(tile) = game::tiles::tile(&e.library.tiles, i.tile) else {
                continue;
            };
            for (n, s) in tile.sockets.iter().enumerate() {
                let port = game::tiles::Port {
                    instance: i.id,
                    socket: n,
                };
                if w.connected(port) {
                    continue;
                }
                let (p, _) = game::tiles::frame(i, s);
                if let Ok(screen) = camera.world_to_viewport(transform, p)
                    && r.contains(screen)
                {
                    sockets.push(serde_json::json!({
                        "piece": i.id, "socket": n, "kind": s.kind,
                        "center": [screen.x, screen.y],
                    }));
                }
            }
        }
    }
    // Move handles: the screen positions of their center and arrow midpoints.
    let mut handles = vec![];
    if let Ok((camera, transform)) = cameras.single() {
        let length = gizmo::arrow_length(&e);
        for (handle, p, axes) in gizmo::handles(&e) {
            let mut parts = vec![("Free".to_string(), p)];
            if axes {
                for axis in 0..3 {
                    parts.push((
                        format!("Axis({axis})"),
                        p + Vec3::AXES[axis] * length * 0.65,
                    ));
                }
            }
            for (part, point) in parts {
                if let Ok(screen) = camera.world_to_viewport(transform, point) {
                    handles.push(serde_json::json!({
                        "handle": format!("{handle:?}"), "part": part,
                        "center": [screen.x, screen.y],
                    }));
                }
            }
        }
    }
    // Scrollable panes, so input tests can wheel content into view.
    let panes: Vec<_> = panes
        .iter()
        .map(|(area, node, t, position)| {
            let r = Rect::from_center_size(t.translation, node.size());
            let max =
                ((node.content_size().y - node.size().y) * node.inverse_scale_factor()).max(0.);
            serde_json::json!({
                "key": area.0,
                "rect": [r.min.x, r.min.y, r.width(), r.height()],
                "offset": position.0.y,
                "max": max,
            })
        })
        .collect();
    let view = e.view();
    let mode = e.active_kind().map_or("None".into(), |k| format!("{k:?}"));
    let state = serde_json::json!({
        "mode": mode,
        "active": e.active,
        "tabs": e.tabs,
        "selection": format!("{:?}", e.sel()),
        "tool": format!("{:?}", e.tool()),
        "revision": e.revision,
        "notice": e.notice.text,
        "notice_kind": format!("{:?}", e.notice.kind),
        "naming": e.editing.as_ref().map(|t| &t.buffer),
        "editing": e.editing.as_ref().map(|t| format!("{:?}", t.target)),
        "menu": e.menu.as_ref().map(|m| format!("{:?}", m.menu)),
        "modal": e.modal.is_some(),
        "workspace": e.workspace_dir,
        "workspace_ready": e.workspace_ready,
        "library": e.library,
        "cursor": view.map(|v| v.cursor),
        "brush": e.brush,
        "color": e.color,
        "animation": e.anim,
        "placement": e.placement,
        "preview_camera": view.map(|v| v.cam),
        "viewport": rect.rect.map(|r| [r.min.x, r.min.y, r.width(), r.height()]),
        "layout": e.layout,
        "dirty": e.dirty(),
        "can_undo": e.can_undo(),
        "window": [width as u32, height as u32],
        "controls": controls,
        "sockets": sockets,
        "panes": panes,
        "handles": handles,
    });
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if let Ok(bytes) = serde_json::to_vec(&state)
        && std::fs::write(&tmp, bytes).is_ok()
    {
        let _ = std::fs::rename(tmp, path);
    }
}
