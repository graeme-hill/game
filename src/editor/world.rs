//! Tile and graph-world editing helpers: socket placement, voxel volumes and
//! world-specific guides. Placement always goes through socket validation.
use super::*;
use game::tiles::*;

/// The open socket targeted for placement, if any.
pub fn target_port(e: &Editor) -> Option<Port> {
    match e.sel() {
        Sel::PieceSocket(instance, socket) => Some(Port { instance, socket }),
        _ => None,
    }
}

/// An instance of `tile` turned `turn` quarter-steps, aligned to the target socket.
pub fn fit(e: &Editor, tile_id: u32, turn: usize) -> Result<Instance, String> {
    let w = e.active_world().ok_or("Open a world")?;
    let t = tile(&e.library.tiles, tile_id)?;
    let r = t.rotations[turn % t.rotations.len()];
    if w.instances.is_empty() {
        return Ok(Instance {
            id: w.next_id(),
            tile: t.id,
            position: [0.; 3],
            rotation: r,
        });
    }
    let p = target_port(e).ok_or("Select an open socket on a placed piece")?;
    if w.connected(p) {
        return Err("That socket is already connected".into());
    }
    let target = port(w, &e.library.tiles, p)?;
    for s in 0..t.sockets.len() {
        let i = aligned(w, &e.library.tiles, p, t, s, r)?;
        if compatible(target, (&i, &t.sockets[s]), &e.library.socket_rules) {
            can_place(w, &e.library.tiles, &i)?;
            return Ok(i);
        }
    }
    Err("That tile has no compatible socket in this orientation".into())
}

pub fn candidate(e: &Editor) -> Result<Instance, String> {
    let tile = e.placement.tile.ok_or("Choose a tile to place")?;
    fit(e, tile, e.placement.turn)
}

pub fn connect(e: &mut Editor, instance: Instance) -> Result<(), String> {
    let tiles = e.library.tiles.clone();
    let rules = e.library.socket_rules.clone();
    let w = e.active_world_mut().ok_or("Open a world")?;
    connect_instance(w, &tiles, &rules, instance)
}

/// Every (tile, turn) that can connect to the selected socket, deduplicated by
/// resulting transform so symmetric tiles are listed once.
pub fn options(e: &Editor) -> Vec<(u32, usize)> {
    let mut found: Vec<(u32, usize, Instance)> = vec![];
    for t in &e.library.tiles {
        for turn in 0..t.rotations.len() {
            if let Ok(i) = fit(e, t.id, turn)
                && !found.iter().any(|(id, _, other)| {
                    *id == t.id
                        && Vec3::from_array(other.position).distance(Vec3::from_array(i.position))
                            < 0.01
                        && Quat::from_array(other.rotation)
                            .angle_between(Quat::from_array(i.rotation))
                            < 0.01
                })
            {
                found.push((t.id, turn, i));
            }
        }
    }
    found.into_iter().map(|(id, turn, _)| (id, turn)).collect()
}

/// Fill the required sockets of the selected piece, and of every piece this
/// places, with fitting tiles (seeded choice). All-or-nothing: an unfillable
/// socket fails the whole operation so the caller's edit is rolled back.
pub fn complete_piece(e: &mut Editor) -> Result<usize, String> {
    let (Sel::Piece(start) | Sel::PieceSocket(start, _)) = e.sel() else {
        return Err("Select a piece to complete".into());
    };
    let mut queue = vec![start];
    let mut placed = 0;
    let mut rng = e
        .generator
        .seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    while let Some(piece) = queue.pop() {
        loop {
            let w = e.active_world().ok_or("Open a world")?;
            let instance = w
                .instances
                .iter()
                .find(|i| i.id == piece)
                .ok_or("Missing piece")?;
            let t = tile(&e.library.tiles, instance.tile)?;
            let Some((socket, name)) = t.sockets.iter().enumerate().find_map(|(n, s)| {
                (s.required
                    && !w.connected(Port {
                        instance: piece,
                        socket: n,
                    }))
                .then(|| (n, s.name.clone()))
            }) else {
                break;
            };
            if placed >= 64 {
                return Err("Gave up after 64 pieces; complete smaller parts".into());
            }
            e.view_mut().sel = Sel::PieceSocket(piece, socket);
            // Prefer tiles that ask for the least, so completion terminates;
            // the seed varies the choice among equals.
            let demands = |id: u32| {
                tile(&e.library.tiles, id).map_or(usize::MAX, |t| {
                    t.sockets.iter().filter(|s| s.required).count()
                })
            };
            let choices = options(e);
            let fewest = choices.iter().map(|(id, _)| demands(*id)).min();
            let choices: Vec<_> = choices
                .into_iter()
                .filter(|(id, _)| Some(demands(*id)) == fewest)
                .collect();
            if choices.is_empty() {
                return Err(format!(
                    "No tile fits required socket {name} of piece #{piece}"
                ));
            }
            rng = rng
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let (tile_id, turn) = choices[(rng >> 33) as usize % choices.len()];
            let next = fit(e, tile_id, turn)?;
            let id = next.id;
            connect(e, next)?;
            queue.push(id);
            placed += 1;
        }
    }
    e.view_mut().sel = Sel::Piece(start);
    Ok(placed)
}

/// Paint (or erase with `None`) an axis-aligned box of voxels in a tile.
pub fn paint(t: &mut Tile, min: [i32; 3], size: i32, color: Option<[f32; 3]>) {
    let max = min.map(|v| v + size);
    let mut next = vec![];
    for v in &t.voxels {
        if (0..3).any(|a| v.max[a] <= min[a] || v.min[a] >= max[a]) {
            next.push(v.clone());
            continue;
        }
        let mut middle = v.clone();
        for a in 0..3 {
            if middle.min[a] < min[a] {
                let mut left = middle.clone();
                left.max[a] = min[a];
                next.push(left);
                middle.min[a] = min[a];
            }
            if middle.max[a] > max[a] {
                let mut right = middle.clone();
                right.min[a] = max[a];
                next.push(right);
                middle.max[a] = max[a];
            }
        }
    }
    if let Some(color) = color {
        next.push(Volume { min, max, color });
    }
    t.voxels = next;
}

/// Ray hit against a tile's voxel volumes (tile-local space): cell and face normal.
pub fn raycast_tile(t: &Tile, origin: Vec3, dir: Vec3) -> Option<([i32; 3], [i32; 3])> {
    let mut best: Option<(f32, usize, i32, &Volume)> = None;
    for v in &t.voxels {
        let mut near = f32::NEG_INFINITY;
        let mut far = f32::INFINITY;
        let mut axis = 0;
        let mut sign = 0;
        let mut hit = true;
        for a in 0..3 {
            let lo = v.min[a] as f32 * UNIT;
            let hi = v.max[a] as f32 * UNIT;
            if dir[a].abs() < 1e-7 {
                if origin[a] < lo || origin[a] > hi {
                    hit = false;
                }
                continue;
            }
            let (t0, t1) = ((lo - origin[a]) / dir[a], (hi - origin[a]) / dir[a]);
            let (enter, exit) = (t0.min(t1), t0.max(t1));
            if enter > near {
                near = enter;
                axis = a;
                sign = if dir[a] > 0. { -1 } else { 1 };
            }
            far = far.min(exit);
        }
        if hit && near <= far && near > 0. && best.is_none_or(|b| near < b.0) {
            best = Some((near, axis, sign, v));
        }
    }
    let (distance, axis, sign, _) = best?;
    // A point just past the entry face lies inside the hit volume.
    let point = origin + dir * (distance + UNIT * 0.01);
    let mut normal = [0; 3];
    normal[axis] = sign;
    Some(((point / UNIT).floor().as_ivec3().to_array(), normal))
}

pub fn preview_bounds(e: &Editor) -> Option<(Vec3, Vec3)> {
    let instances = match e.active_kind()? {
        DocKind::Tile => {
            let t = e.active_tile()?;
            vec![Instance {
                id: 0,
                tile: t.id,
                position: [0.; 3],
                rotation: t.rotations[e.view()?.turn % t.rotations.len()],
            }]
        }
        DocKind::World => e.active_world()?.instances.clone(),
        _ => return None,
    };
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for i in instances {
        if let Ok(t) = tile(&e.library.tiles, i.tile) {
            for b in t.occupied() {
                let b = world_bounds(&i, &b);
                min = min.min(Vec3::from_array(b.min));
                max = max.max(Vec3::from_array(b.max));
            }
        }
    }
    min.is_finite().then_some((min, max))
}

/// One box around everything a tile occupies, in tile space.
pub fn local_bounds(t: &Tile) -> Option<Bounds> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for b in t.occupied() {
        min = min.min(Vec3::from_array(b.min));
        max = max.max(Vec3::from_array(b.max));
    }
    min.is_finite().then(|| Bounds {
        min: min.to_array(),
        max: max.to_array(),
    })
}

fn box_gizmo(
    g: &mut Gizmos<viewport::OverlayGuides>,
    transform: Transform,
    b: &Bounds,
    color: Color,
) {
    let local =
        Transform::from_translation((Vec3::from_array(b.min) + Vec3::from_array(b.max)) * 0.5)
            .with_scale(Vec3::from_array(b.max) - Vec3::from_array(b.min));
    g.cube(transform.mul_transform(local), color);
}

pub fn guides(
    pointer: Res<input::EditorPointer>,
    target_hover: Res<viewport::ViewportHover>,
    e: Res<Editor>,
    mut g: Gizmos<viewport::OverlayGuides>,
    hover: Res<viewport::HoverCell>,
) {
    if !e.guides {
        return;
    }
    let theme = ui::theme::Palette::get();
    match e.active_kind() {
        Some(DocKind::Tile) => {
            let Some(t) = e.active_tile() else { return };
            let view = e.view().unwrap();
            let i = Instance {
                id: 0,
                tile: t.id,
                position: [0.; 3],
                rotation: t.rotations[view.turn % t.rotations.len()],
            };
            for (n, s) in t.sockets.iter().enumerate() {
                let selected = view.sel == Sel::Socket(n) || target_hover.0 == Some(Sel::Socket(n));
                socket_marker(&mut g, &i, s, selected, s.required, &theme);
            }
            for (n, b) in t.colliders.iter().enumerate() {
                let selected = view.sel == Sel::Collider(n);
                if selected || view.tool == Tool::Collider {
                    box_gizmo(
                        &mut g,
                        i.transform(),
                        b,
                        if selected {
                            theme.warning
                        } else {
                            theme.warning.with_alpha(0.45)
                        },
                    );
                }
            }
            let size = e.brush as f32 * UNIT;
            let cursor = Vec3::from_array(view.cursor.map(|v| v as f32 * UNIT));
            if matches!(view.tool, Tool::Socket | Tool::Collider) {
                g.cube(
                    i.transform().mul_transform(
                        Transform::from_translation(cursor + Vec3::splat(size * 0.5))
                            .with_scale(Vec3::splat(size)),
                    ),
                    Color::WHITE.with_alpha(0.7),
                );
            }
            if let Some(cell) = hover.cell
                && matches!(view.tool, Tool::Paint | Tool::Erase)
            {
                let color = if view.tool == Tool::Erase {
                    theme.danger
                } else {
                    Color::WHITE
                };
                g.cube(
                    i.transform().mul_transform(
                        Transform::from_translation(
                            Vec3::from_array(cell.map(|v| v as f32 * UNIT))
                                + Vec3::splat(size * 0.5),
                        )
                        .with_scale(Vec3::splat(size * 1.01)),
                    ),
                    color,
                );
            }
        }
        Some(DocKind::World) => {
            let Some(w) = e.active_world() else { return };
            let sel = e.sel();
            let selected_piece = match sel {
                Sel::Piece(id) | Sel::PieceSocket(id, _) => Some(id),
                _ => None,
            };
            // Open sockets everywhere are click targets; connected ones stay quiet.
            for i in &w.instances {
                let Ok(t) = tile(&e.library.tiles, i.tile) else {
                    continue;
                };
                for (n, s) in t.sockets.iter().enumerate() {
                    let p = Port {
                        instance: i.id,
                        socket: n,
                    };
                    let connected = w.connected(p);
                    let selected = sel == Sel::PieceSocket(i.id, n)
                        || target_hover.0 == Some(Sel::PieceSocket(i.id, n));
                    if connected && !selected && selected_piece != Some(i.id) {
                        continue;
                    }
                    let (pos, _) = frame(i, s);
                    if connected {
                        g.sphere(Isometry3d::from_translation(pos), 0.12, theme.text_muted);
                    } else if selected_piece == Some(i.id) || selected {
                        socket_marker(&mut g, i, s, selected, s.required, &theme);
                    } else {
                        // Quiet click targets until their piece is selected.
                        let color = if s.required {
                            theme.socket_required
                        } else {
                            theme.socket
                        };
                        g.sphere(
                            Isometry3d::from_translation(pos),
                            0.1,
                            color.with_alpha(0.75),
                        );
                    }
                }
                if let Some(b) = local_bounds(t) {
                    if selected_piece == Some(i.id) {
                        box_gizmo(&mut g, i.transform(), &b, theme.accent);
                    } else if target_hover.0 == Some(Sel::Piece(i.id)) {
                        box_gizmo(&mut g, i.transform(), &b, Color::WHITE.with_alpha(0.5));
                    }
                }
            }
            // A hovered menu entry previews its placement; otherwise the
            // Inspector's chosen tile does.
            let placed = match pointer.preview {
                Some((tile, turn)) => fit(&e, tile, turn),
                None => candidate(&e),
            };
            if let Ok(i) = placed
                && let Ok(t) = tile(&e.library.tiles, i.tile)
                && let Some(b) = local_bounds(t)
            {
                box_gizmo(&mut g, i.transform(), &b, theme.success);
            }
        }
        _ => {}
    }
}

fn socket_marker(
    g: &mut Gizmos<viewport::OverlayGuides>,
    i: &Instance,
    s: &Socket,
    selected: bool,
    required: bool,
    theme: &ui::theme::Palette,
) {
    let (p, r) = frame(i, s);
    let color = if selected {
        theme.warning
    } else if required {
        theme.socket_required
    } else {
        theme.socket
    };
    let radius = if selected { 0.22 } else { 0.15 };
    g.sphere(Isometry3d::from_translation(p), radius, color);
    g.arrow(p, p + r * Vec3::Z * 0.9, color);
}
