//! The 3D viewport: camera, preview geometry, guides and pointer tools.
//! The viewport fills whatever rectangle the UI layout reserves for it.
use super::*;
use crate::editor::input::EditorPointer;
use bevy::{
    camera::{Viewport, visibility::RenderLayers},
    input::mouse::{MouseMotion, MouseWheel},
};
use game::{
    animation::{BoneFrame, blend_clips, forward_kinematics},
    character_render::CharacterRenderer,
    tiles::{self, UNIT},
    voxel::{self, VOXEL_SIZE},
};

#[derive(Component)]
pub struct Preview;
#[derive(Component)]
pub struct PreviewCamera;
/// UI node whose rectangle the 3D camera renders into.
#[derive(Component)]
pub struct ViewportSlot;
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct OverlayGuides;

/// The viewport's rectangle in physical pixels, when a 3D document is shown.
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct ViewportRect {
    pub rect: Option<Rect>,
}
impl ViewportRect {
    pub fn contains(&self, p: Vec2) -> bool {
        self.rect.is_some_and(|r| r.contains(p))
    }
}

/// What a click in the viewport would select right now (hover highlight).
#[derive(Resource, Default)]
pub struct ViewportHover(pub Option<Sel>);

/// The voxel cell under the cursor for paint/erase previews.
#[derive(Resource, Default)]
pub struct HoverCell {
    pub cell: Option<[i32; 3]>,
}

pub fn setup(mut commands: Commands, mut gizmo_config: ResMut<GizmoConfigStore>) {
    // Editing guides stay visible through the SDF surface and voxels.
    let (config, _) = gizmo_config.config_mut::<OverlayGuides>();
    config.depth_bias = -1.0;
    config.line.width = 2.0;
    let (config, _) = gizmo_config.config_mut::<DefaultGizmoConfigGroup>();
    config.line.width = 1.0;
    let theme = ui::theme::Palette::get();
    commands.spawn((
        Camera3d::default(),
        Camera {
            clear_color: ClearColorConfig::Custom(theme.viewport),
            is_active: false,
            ..default()
        },
        PreviewCamera,
        Transform::from_xyz(3., 2.5, 6.).looking_at(Vec3::Y, Vec3::Y),
    ));
    commands.spawn((
        Camera2d,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        IsDefaultUiCamera,
        RenderLayers::layer(1),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.,
            ..default()
        },
        Transform::from_xyz(3., 7., 5.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

/// Fit the camera to the active document.
pub fn frame(e: &mut Editor) {
    let Some(kind) = e.active_kind() else { return };
    let (center, extent) = match kind {
        DocKind::Body | DocKind::Character => e
            .preview_body()
            .and_then(|b| b.bounds())
            .map_or((Vec3::Y, 2.), |(a, b)| {
                ((a + b) * 0.5, (b - a).max_element())
            }),
        DocKind::Prop => e
            .active_prop()
            .and_then(prop_bounds)
            .map_or((Vec3::new(1.6, 0.4, 1.6), 3.2), |(min, max)| {
                ((min + max) * 0.5, (max - min).max_element().max(1.2))
            }),
        DocKind::Tile | DocKind::World => world::preview_bounds(e)
            .map_or((Vec3::ZERO, 12.8), |(a, b)| {
                ((a + b) * 0.5, (b - a).max_element())
            }),
        DocKind::Rules => return,
    };
    let distance = match kind {
        DocKind::World => (extent * 1.15).clamp(4., 400.),
        DocKind::Tile => (extent * 1.6).clamp(3., 120.),
        // Leave room for equipped props and the armature guides.
        DocKind::Body | DocKind::Character => (extent * 2.2 + 1.3).clamp(2.2, 30.),
        _ => (extent * 2.1).clamp(2.2, 30.),
    };
    // Narrow viewports need more distance to fit the same width.
    let distance = distance / (e.aspect / 1.4).clamp(0.45, 1.);
    let cam = &mut e.view_mut().cam;
    cam.pan = center.to_array();
    cam.distance = distance;
}

fn limits(kind: Option<DocKind>) -> (f32, f32) {
    match kind {
        Some(DocKind::World) => (2., 600.),
        Some(DocKind::Tile) => (1., 150.),
        _ => (0.6, 40.),
    }
}

/// Place the 3D camera in the rectangle the layout reserved for it.
pub fn sync_viewport(
    slots: Query<(&ComputedNode, &UiGlobalTransform), With<ViewportSlot>>,
    mut cameras: Query<&mut Camera, With<PreviewCamera>>,
    mut rect: ResMut<ViewportRect>,
    mut e: ResMut<Editor>,
) {
    let shown = e.workspace_ready && e.active_kind().is_some_and(|k| k != DocKind::Rules);
    let slot = slots.iter().next().filter(|_| shown).and_then(|(node, t)| {
        let size = node.size();
        (size.x >= 2. && size.y >= 2.).then(|| Rect::from_center_size(t.translation, size))
    });
    if rect.rect != slot {
        rect.rect = slot;
        if let Some(r) = slot {
            e.bypass_change_detection().aspect = r.width() / r.height().max(1.);
        }
    }
    for mut camera in &mut cameras {
        match slot {
            Some(r) => {
                let viewport = Viewport {
                    physical_position: r.min.round().as_uvec2(),
                    physical_size: r.size().round().as_uvec2().max(UVec2::ONE),
                    ..default()
                };
                if camera
                    .viewport
                    .as_ref()
                    .map(|v| (v.physical_position, v.physical_size))
                    != Some((viewport.physical_position, viewport.physical_size))
                {
                    camera.viewport = Some(viewport);
                }
                if !camera.is_active {
                    camera.is_active = true;
                }
            }
            None => {
                if camera.is_active {
                    camera.is_active = false;
                }
            }
        }
    }
}

pub fn camera(e: Res<Editor>, mut cameras: Query<&mut Transform, With<PreviewCamera>>) {
    if !e.is_changed() {
        return;
    }
    let cam = e.cam();
    let center = Vec3::from_array(cam.pan);
    let horizontal = cam.pitch.cos();
    let offset = Vec3::new(
        cam.yaw.sin() * horizontal,
        cam.pitch.sin(),
        cam.yaw.cos() * horizontal,
    ) * cam.distance;
    for mut t in &mut cameras {
        *t = Transform::from_translation(center + offset).looking_at(center, Vec3::Y);
    }
}

/// Right-drag orbits, middle-drag (or shift+right) pans, wheel zooms.
#[allow(clippy::too_many_arguments)]
pub fn camera_input(
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    windows: Single<&Window>,
    rect: Res<ViewportRect>,
    mut pointer: ResMut<EditorPointer>,
    mut e: ResMut<Editor>,
) {
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    let scroll: f32 = wheel
        .read()
        .map(|w| match w.unit {
            bevy::input::mouse::MouseScrollUnit::Line => w.y,
            bevy::input::mouse::MouseScrollUnit::Pixel => w.y / 40.,
        })
        .sum();
    let cursor = windows.cursor_position();
    let inside = cursor.is_some_and(|p| rect.contains(p)) && !pointer.over_ui;
    if e.active.is_none() || e.active_kind() == Some(DocKind::Rules) {
        pointer.camera_drag = None;
        return;
    }
    for button in [MouseButton::Right, MouseButton::Middle] {
        if mouse.just_pressed(button) && inside && e.menu.is_none() {
            pointer.camera_drag = Some(button);
        }
    }
    if pointer
        .camera_drag
        .is_some_and(|button| !mouse.pressed(button))
    {
        pointer.camera_drag = None;
    }
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let kind = e.active_kind();
    match pointer.camera_drag {
        Some(MouseButton::Right) if delta != Vec2::ZERO && !shift => {
            let cam = e.cam_mut().unwrap();
            cam.yaw -= delta.x * 0.008;
            cam.pitch = (cam.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
        }
        Some(MouseButton::Middle | MouseButton::Right) if delta != Vec2::ZERO => {
            let height = rect.rect.map_or(600., |r| r.height()).max(1.);
            let cam = e.cam_mut().unwrap();
            let scale = cam.distance * 0.9 / height;
            let right = Vec3::new(cam.yaw.cos(), 0., -cam.yaw.sin());
            let forward = -Vec3::new(
                cam.yaw.sin() * cam.pitch.cos(),
                cam.pitch.sin(),
                cam.yaw.cos() * cam.pitch.cos(),
            );
            let up = right.cross(forward);
            let pan = Vec3::from_array(cam.pan) - right * delta.x * scale + up * delta.y * scale;
            cam.pan = pan.to_array();
        }
        _ => {}
    }
    if scroll != 0. && inside && e.menu.is_none() {
        let (lo, hi) = limits(kind);
        let cam = e.cam_mut().unwrap();
        cam.distance = (cam.distance * 0.88f32.powf(scroll)).clamp(lo, hi);
    }
}

/// Posed bone frames for the preview, matching the animation preview system.
pub fn preview_frames(e: &Editor) -> Option<Vec<BoneFrame>> {
    let body = e.preview_body()?;
    let mut clips = vec![];
    if let Some(clip) = e.animation_clip() {
        let mut clip = clip.clone();
        clip.looping = clip.looping && e.anim.playing;
        clips.push((clip, e.anim.time, 1.));
        if let Some(other) = e
            .anim
            .blend_target
            .and_then(|i| e.active_character()?.animations.get(i))
        {
            let mut other = other.clone();
            other.looping = other.looping && e.anim.playing;
            let time = e.anim.time / clips[0].0.duration * other.duration;
            clips[0].2 = 1. - e.anim.blend;
            clips.push((other, time, e.anim.blend));
        }
    }
    let refs: Vec<_> = clips.iter().map(|(c, t, w)| (c, *t, *w)).collect();
    Some(forward_kinematics(body, &blend_clips(body, &refs)))
}

pub fn rebuild(
    mut renderer: CharacterRenderer,
    e: Res<Editor>,
    old: Query<Entity, With<Preview>>,
    mut last: Local<u64>,
) {
    if *last == e.scene_revision {
        return;
    }
    *last = e.scene_revision;
    for entity in &old {
        renderer.commands.entity(entity).despawn();
    }
    let entities = match e.active_kind() {
        None | Some(DocKind::Rules) => vec![],
        Some(DocKind::Tile) => e
            .active_tile()
            .map(|t| {
                let turn = e.view().map_or(0, |v| v.turn);
                renderer.tile(
                    t,
                    Transform::from_rotation(Quat::from_array(
                        t.rotations[turn % t.rotations.len()],
                    )),
                )
            })
            .unwrap_or_default(),
        Some(DocKind::World) => {
            let mut entities = vec![];
            if let Some(w) = e.active_world() {
                for i in &w.instances {
                    if let Ok(t) = tiles::tile(&e.library.tiles, i.tile) {
                        entities.extend(renderer.tile(t, i.transform()));
                    }
                }
            }
            entities
        }
        Some(DocKind::Prop) => e
            .active_prop()
            .and_then(|prop| renderer.prop(prop, Transform::default()))
            .into_iter()
            .collect(),
        Some(DocKind::Character) => e
            .active_character()
            .map(|c| renderer.character(&e.library, c, Vec3::ZERO))
            .unwrap_or_default(),
        Some(DocKind::Body) => e
            .active_body()
            .map(|body| vec![renderer.body(body, Vec3::ZERO)])
            .unwrap_or_default(),
    };
    for entity in entities {
        renderer.commands.entity(entity).insert(Preview);
    }
}

#[allow(clippy::too_many_arguments)]
pub fn guides(
    e: Res<Editor>,
    hover: Res<HoverCell>,
    gizmo_hover: Res<gizmo::GizmoHover>,
    target_hover: Res<ViewportHover>,
    pointer: Res<EditorPointer>,
    cameras: Query<(&GlobalTransform, &Projection), With<PreviewCamera>>,
    mut g: Gizmos,
    mut overlay: Gizmos<OverlayGuides>,
) {
    let Some(kind) = e.active_kind() else { return };
    // Move handles stay available with guides hidden.
    let active = match pointer.viewport_drag {
        Some(ViewportDrag::Gizmo(drag)) => Some(drag),
        _ => None,
    };
    gizmo::draw(&e, gizmo_hover.0, active, &mut overlay);
    if kind == DocKind::Rules {
        return;
    }
    if let Ok((camera, Projection::Perspective(perspective))) = cameras.single() {
        axis_indicator(&mut overlay, camera, perspective.fov, e.aspect);
    }
    if !e.guides {
        return;
    }
    let theme = ui::theme::Palette::get();
    match kind {
        DocKind::Body | DocKind::Character => {
            ground_grid(&mut g, 0.25, 12, &theme);
            let Some(body) = e.preview_body() else { return };
            let frames = preview_frames(&e).unwrap_or_default();
            let sel = e.sel();
            let show_all =
                kind == DocKind::Body || matches!(sel, Sel::Bone(_)) || e.anim.clip.is_some();
            let hovered = target_hover.0;
            for (bone, f) in body.bones.iter().zip(&frames) {
                let selected = sel == Sel::Bone(bone.id);
                let lit = hovered == Some(Sel::Bone(bone.id));
                if !show_all && !selected && !lit {
                    continue;
                }
                let color = if selected {
                    theme.accent
                } else if lit {
                    theme.warning
                } else {
                    Color::WHITE.with_alpha(0.75)
                };
                overlay.line(f.start, f.end, color);
                overlay.sphere(
                    Isometry3d::from_translation(f.end),
                    if selected { 0.045 } else { 0.03 },
                    color,
                );
            }
            for m in &body.mounts {
                let Some(index) = body.bones.iter().position(|b| b.id == m.bone) else {
                    continue;
                };
                let Some(f) = frames.get(index) else { continue };
                let selected = sel == Sel::Mount(m.id)
                    || (kind == DocKind::Character && e.equip.mount == Some(m.id))
                    || hovered == Some(Sel::Mount(m.id));
                let rotation = f.rotation * game::animation::euler(m.rotation);
                let p = f.end + f.rotation * Vec3::from_array(m.offset);
                axes(
                    &mut overlay,
                    p,
                    rotation,
                    if selected { 0.22 } else { 0.12 },
                    selected,
                    &theme,
                );
            }
            // Outline the selected equipped prop, following the posed mount.
            if let (Sel::Attachment(id), Some(c)) = (sel, e.active_character())
                && let Some(a) = c.attachments.iter().find(|a| a.id == id)
                && let Some(prop) = e.library.props.iter().find(|p| p.id == a.prop)
                && let Some(anchor) = prop.anchors.iter().find(|v| v.id == a.anchor)
                && let Some(mount) = body.mounts.iter().find(|m| m.id == a.mount)
                && let Some(index) = body.bones.iter().position(|b| b.id == mount.bone)
                && let Some(f) = frames.get(index)
                && let Some((lo, hi)) = prop_bounds(prop)
            {
                let euler = game::animation::euler;
                let mount_rotation = f.rotation * euler(mount.rotation);
                let rotation =
                    mount_rotation * euler(a.rotation) * euler(anchor.rotation).inverse();
                let target = f.end
                    + f.rotation * Vec3::from_array(mount.offset)
                    + mount_rotation * Vec3::from_array(a.offset);
                let placed = Transform {
                    translation: target - rotation * (Vec3::from_array(anchor.position) * a.scale),
                    rotation,
                    scale: Vec3::splat(a.scale),
                };
                overlay.cube(
                    placed.mul_transform(
                        Transform::from_translation((lo + hi) * 0.5).with_scale(hi - lo),
                    ),
                    theme.accent,
                );
            }
        }
        DocKind::Prop => {
            let view = e.view().unwrap();
            let y = view.cursor[1] as f32 * VOXEL_SIZE;
            let extent = 32. * VOXEL_SIZE;
            for i in 0..=32 {
                let p = i as f32 * VOXEL_SIZE;
                let color = if i % 8 == 0 {
                    theme.grid_major
                } else {
                    theme.grid
                };
                g.line(Vec3::new(p, y, 0.), Vec3::new(p, y, extent), color);
                g.line(Vec3::new(0., y, p), Vec3::new(extent, y, p), color);
            }
            if e.mirror {
                let x = extent * 0.5;
                for (a, b) in [
                    (Vec3::new(x, 0., 0.), Vec3::new(x, extent, 0.)),
                    (Vec3::new(x, extent, 0.), Vec3::new(x, extent, extent)),
                    (Vec3::new(x, extent, extent), Vec3::new(x, 0., extent)),
                    (Vec3::new(x, 0., extent), Vec3::new(x, 0., 0.)),
                ] {
                    overlay.line(a, b, theme.axis_x.with_alpha(0.8));
                }
            }
            // The editable volume.
            g.cube(
                Transform::from_translation(Vec3::splat(extent * 0.5))
                    .with_scale(Vec3::splat(extent)),
                theme.grid,
            );
            let size = e.brush as f32 * VOXEL_SIZE;
            if let Some(cell) = hover.cell
                && matches!(view.tool, Tool::Paint | Tool::Erase)
            {
                let color = if view.tool == Tool::Erase {
                    theme.danger
                } else {
                    Color::WHITE
                };
                let mut cells = vec![cell];
                if e.mirror {
                    cells.push([32 - cell[0] - e.brush as i32, cell[1], cell[2]]);
                }
                for (n, cell) in cells.into_iter().enumerate() {
                    overlay.cube(
                        Transform::from_translation(
                            Vec3::from_array(cell.map(|v| v as f32 * VOXEL_SIZE))
                                + Vec3::splat(size * 0.5),
                        )
                        .with_scale(Vec3::splat(size * 1.01)),
                        if n == 0 {
                            color
                        } else {
                            color.with_alpha(0.45)
                        },
                    );
                }
            } else {
                let center = Vec3::from_array(view.cursor.map(|v| v as f32 * VOXEL_SIZE))
                    + Vec3::splat(size * 0.5);
                overlay.cube(
                    Transform::from_translation(center).with_scale(Vec3::splat(size)),
                    Color::WHITE.with_alpha(0.5),
                );
            }
            if let Some(prop) = e.active_prop() {
                for a in &prop.anchors {
                    let selected =
                        view.sel == Sel::Anchor(a.id) || target_hover.0 == Some(Sel::Anchor(a.id));
                    axes(
                        &mut overlay,
                        Vec3::from_array(a.position),
                        game::animation::euler(a.rotation),
                        if selected { 0.3 } else { 0.18 },
                        selected,
                        &theme,
                    );
                }
            }
        }
        DocKind::Tile => {
            ground_grid(&mut g, 0.8, 16, &theme);
            if e.mirror {
                let h = 12.8;
                overlay.line(Vec3::new(0., 0., -h), Vec3::new(0., 0., h), theme.axis_x);
                overlay.line(
                    Vec3::new(0., 0., -h),
                    Vec3::new(0., h * 0.5, -h),
                    theme.axis_x.with_alpha(0.5),
                );
                overlay.line(
                    Vec3::new(0., 0., h),
                    Vec3::new(0., h * 0.5, h),
                    theme.axis_x.with_alpha(0.5),
                );
            }
        }
        _ => {}
    }
}

/// World axes drawn just in front of the camera in the viewport's lower
/// right corner, so the view orientation is always readable.
fn axis_indicator(g: &mut Gizmos<OverlayGuides>, camera: &GlobalTransform, fov: f32, aspect: f32) {
    let theme = ui::theme::Palette::get();
    let (_, rotation, position) = camera.to_scale_rotation_translation();
    let depth = 1.;
    let half_height = (fov * 0.5).tan() * depth;
    let half_width = half_height * aspect;
    let size = half_height * 0.11;
    let local = Vec3::new(half_width - size * 1.8, -half_height + size * 1.8, -depth);
    let center = position + rotation * local;
    for axis in 0..3 {
        let dir = Vec3::AXES[axis];
        let color = theme.axis(axis);
        g.line(center, center + dir * size, color);
        g.sphere(
            Isometry3d::from_translation(center + dir * size),
            size * 0.13,
            color,
        );
        g.line(center, center - dir * size * 0.6, color.with_alpha(0.3));
    }
}

/// Occupied voxel bounds of a prop in its local space.
fn prop_bounds(prop: &Prop) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for b in &prop.blocks {
        let lo = Vec3::from_array(b.min.map(|v| v as f32 * VOXEL_SIZE));
        min = min.min(lo);
        max = max.max(lo + Vec3::splat(b.size as f32 * VOXEL_SIZE));
    }
    min.is_finite().then_some((min, max))
}

fn ground_grid(g: &mut Gizmos, spacing: f32, half: i32, theme: &ui::theme::Palette) {
    let extent = spacing * half as f32;
    for i in -half..=half {
        let f = i as f32 * spacing;
        let (cx, cz) = if i == 0 {
            (theme.axis_z.with_alpha(0.7), theme.axis_x.with_alpha(0.7))
        } else if i % 4 == 0 {
            (theme.grid_major, theme.grid_major)
        } else {
            (theme.grid, theme.grid)
        };
        g.line(Vec3::new(f, 0., -extent), Vec3::new(f, 0., extent), cx);
        g.line(Vec3::new(-extent, 0., f), Vec3::new(extent, 0., f), cz);
    }
}

fn axes(
    g: &mut Gizmos<OverlayGuides>,
    p: Vec3,
    rotation: Quat,
    length: f32,
    selected: bool,
    theme: &ui::theme::Palette,
) {
    for (axis, color) in [
        (Vec3::X, theme.axis_x),
        (Vec3::Y, theme.axis_y),
        (Vec3::Z, theme.axis_z),
    ] {
        g.line(p, p + rotation * axis * length, color);
    }
    let ring = if selected {
        theme.accent
    } else {
        Color::WHITE.with_alpha(0.8)
    };
    g.sphere(Isometry3d::new(p, rotation), length * 0.25, ring);
}

/// What a left press in the viewport turned into.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewportDrag {
    /// A click that becomes an orbit if the pointer moves.
    Pick { start: Vec2, orbiting: bool },
    /// Dragging a move handle.
    Gizmo(gizmo::GizmoDrag),
    /// Painting on a fixed plane so dragging spreads sideways, not toward the camera.
    Paint {
        axis: usize,
        layer: i32,
        erase: bool,
        last: [i32; 3],
    },
}

fn cursor_ray(
    window: &Window,
    camera: &Camera,
    t: &GlobalTransform,
    rect: &ViewportRect,
) -> Option<Ray3d> {
    let cursor = window.cursor_position()?;
    if !rect.contains(cursor) {
        return None;
    }
    // Camera conversions work in window coordinates.
    camera.viewport_to_world(t, cursor).ok()
}

/// The cell a paint/erase click would affect, and the plane it lies on.
/// The color of the voxel under the ray (the eyedropper).
fn pick_color(e: &Editor, ray: Ray3d) -> Option<[f32; 3]> {
    let (origin, dir) = (ray.origin, *ray.direction);
    match e.active_kind()? {
        DocKind::Prop => {
            let prop = e.active_prop()?;
            let hit = voxel::raycast(prop, origin, dir)?;
            prop.blocks
                .iter()
                .find(|b| b.contains(hit.cell))
                .map(|b| b.color)
        }
        DocKind::Tile => {
            let t = e.active_tile()?;
            let r = Quat::from_array(t.rotations[e.view()?.turn % t.rotations.len()]).inverse();
            let (cell, _) = world::raycast_tile(t, r * origin, r * dir)?;
            // Later volumes paint over earlier ones.
            t.voxels
                .iter()
                .rev()
                .find(|v| (0..3).all(|a| cell[a] >= v.min[a] && cell[a] < v.max[a]))
                .map(|v| v.color)
        }
        _ => None,
    }
}

fn voxel_target(e: &Editor, ray: Ray3d, erase: bool) -> Option<([i32; 3], usize)> {
    let view = e.view()?;
    let (origin, dir) = (ray.origin, *ray.direction);
    let hit = match e.active_kind()? {
        DocKind::Prop => voxel::raycast(e.active_prop()?, origin, dir).map(|h| (h.cell, h.normal)),
        DocKind::Tile => {
            let t = e.active_tile()?;
            let r = Quat::from_array(t.rotations[view.turn % t.rotations.len()]).inverse();
            world::raycast_tile(t, r * origin, r * dir)
        }
        _ => return None,
    };
    let unit = if e.active_kind() == Some(DocKind::Prop) {
        VOXEL_SIZE
    } else {
        UNIT
    };
    let (cell, axis) = match hit {
        Some((mut cell, normal)) => {
            let axis = normal.iter().position(|v| *v != 0).unwrap_or(1);
            if !erase {
                for a in 0..3 {
                    cell[a] += normal[a];
                }
            }
            (cell, axis)
        }
        None => {
            // Empty space: the horizontal layer at the cursor's Y.
            if dir.y.abs() < 1e-4 {
                return None;
            }
            let y = view.cursor[1] as f32 * unit;
            let distance = (y - origin.y) / dir.y;
            if distance < 0. {
                return None;
            }
            let p = origin + dir * distance;
            let mut cell = (p / unit).floor().as_ivec3().to_array();
            cell[1] = view.cursor[1];
            (cell, 1)
        }
    };
    snap_cell(e, cell).map(|c| (c, axis))
}

/// Align a cell to the brush grid and keep it inside the editable bounds.
fn snap_cell(e: &Editor, cell: [i32; 3]) -> Option<[i32; 3]> {
    let brush = e.brush as i32;
    if e.active_kind() == Some(DocKind::Prop) {
        if cell.iter().any(|v| !(0..32).contains(v)) {
            return None;
        }
        Some(cell.map(|v| (v.div_euclid(brush) * brush).clamp(0, 32 - brush)))
    } else {
        Some(cell.map(|v| v.div_euclid(brush) * brush))
    }
}

/// Cell on a fixed axis-aligned plane under the cursor (for drag painting).
fn plane_cell(e: &Editor, ray: Ray3d, axis: usize, layer: i32) -> Option<[i32; 3]> {
    let view = e.view()?;
    let (mut origin, mut dir) = (ray.origin, *ray.direction);
    let unit = if e.active_kind() == Some(DocKind::Prop) {
        VOXEL_SIZE
    } else {
        let t = e.active_tile()?;
        let r = Quat::from_array(t.rotations[view.turn % t.rotations.len()]).inverse();
        origin = r * origin;
        dir = r * dir;
        UNIT
    };
    if dir[axis].abs() < 1e-4 {
        return None;
    }
    let plane = (layer as f32 + 0.5) * unit;
    let distance = (plane - origin[axis]) / dir[axis];
    if distance < 0. {
        return None;
    }
    let mut cell = ((origin + dir * distance) / unit)
        .floor()
        .as_ivec3()
        .to_array();
    cell[axis] = layer;
    snap_cell(e, cell)
}

/// Distance along a ray to an axis-aligned box, if it hits.
fn ray_box(origin: Vec3, dir: Vec3, min: [f32; 3], max: [f32; 3]) -> Option<f32> {
    let mut near = 0f32;
    let mut far = f32::INFINITY;
    for a in 0..3 {
        if dir[a].abs() < 1e-7 {
            if origin[a] < min[a] || origin[a] > max[a] {
                return None;
            }
            continue;
        }
        let (u, v) = ((min[a] - origin[a]) / dir[a], (max[a] - origin[a]) / dir[a]);
        near = near.max(u.min(v));
        far = far.min(u.max(v));
    }
    (near <= far).then_some(near)
}

/// Move the voxel cursor to the cell in front of the clicked face.
fn move_cursor(e: &mut Editor, ray: Option<Ray3d>) {
    if let Some((cell, _)) = ray.and_then(|r| voxel_target(e, r, false)) {
        e.view_mut().cursor = cell;
        e.changed();
    }
}

/// Bone under the cursor (screen-space distance to its posed segment).
fn pick_bone(e: &Editor, camera: &Camera, t: &GlobalTransform, cursor: Vec2) -> Option<u32> {
    let body = e.preview_body()?;
    let frames = preview_frames(e)?;
    let mut best: Option<(u32, f32)> = None;
    for (bone, f) in body.bones.iter().zip(&frames) {
        let (Ok(a), Ok(b)) = (
            camera.world_to_viewport(t, f.start),
            camera.world_to_viewport(t, f.end),
        ) else {
            continue;
        };
        let ab = b - a;
        let s = if ab.length_squared() > 0. {
            ((cursor - a).dot(ab) / ab.length_squared()).clamp(0., 1.)
        } else {
            0.
        };
        let d = cursor.distance(a.lerp(b, s));
        if d < best.map_or(26., |(_, v)| v) {
            best = Some((bone.id, d));
        }
    }
    best.map(|(id, _)| id)
}

/// Mount under the cursor in a body or character.
fn pick_mount(e: &Editor, camera: &Camera, t: &GlobalTransform, cursor: Vec2) -> Option<u32> {
    let body = e.preview_body()?;
    let frames = preview_frames(e)?;
    body.mounts.iter().find_map(|m| {
        let index = body.bones.iter().position(|b| b.id == m.bone)?;
        let f = frames.get(index)?;
        let p = camera
            .world_to_viewport(t, f.end + f.rotation * Vec3::from_array(m.offset))
            .ok()?;
        (p.distance(cursor) < 12.).then_some(m.id)
    })
}

/// Socket or piece under the cursor in a world.
fn pick_world(e: &Editor, camera: &Camera, t: &GlobalTransform, cursor: Vec2, ray: Ray3d) -> Sel {
    let Some(w) = e.active_world() else {
        return Sel::None;
    };
    let mut best: Option<(Sel, f32)> = None;
    for i in &w.instances {
        let Ok(tile) = tiles::tile(&e.library.tiles, i.tile) else {
            continue;
        };
        for (n, s) in tile.sockets.iter().enumerate() {
            if w.connected(tiles::Port {
                instance: i.id,
                socket: n,
            }) {
                continue;
            }
            let (p, _) = tiles::frame(i, s);
            if let Ok(screen) = camera.world_to_viewport(t, p) {
                let d = screen.distance(cursor);
                if d < best.map_or(14., |(_, v)| v) {
                    best = Some((Sel::PieceSocket(i.id, n), d));
                }
            }
        }
    }
    if let Some((sel, _)) = best {
        return sel;
    }
    let mut nearest = (f32::INFINITY, Sel::None);
    for i in &w.instances {
        let Ok(tile) = tiles::tile(&e.library.tiles, i.tile) else {
            continue;
        };
        let Some(b) = world::local_bounds(tile) else {
            continue;
        };
        let b = tiles::world_bounds(i, &b);
        let Some(near) = ray_box(ray.origin, *ray.direction, b.min, b.max) else {
            continue;
        };
        // Prefer the smallest enclosing piece: trees and houses sit on plots.
        let size = Vec3::from_array(b.max) - Vec3::from_array(b.min);
        let score = near + size.length() * 0.05;
        if score < nearest.0 {
            nearest = (score, Sel::Piece(i.id));
        }
    }
    nearest.1
}

/// Left-button tools inside the viewport: picking, painting, orbit-drag.
#[allow(clippy::too_many_arguments)]
pub fn pointer_tools(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Single<&Window>,
    cameras: Single<(&Camera, &GlobalTransform), With<PreviewCamera>>,
    rect: Res<ViewportRect>,
    mut pointer: ResMut<EditorPointer>,
    mut hover: ResMut<HoverCell>,
    mut gizmo_hover: ResMut<gizmo::GizmoHover>,
    mut target_hover: ResMut<ViewportHover>,
    keys: Res<ButtonInput<KeyCode>>,
    mut e: ResMut<Editor>,
    options: Res<LaunchOptions>,
    ui_scale: Res<UiScale>,
) {
    let (camera, transform) = *cameras;
    let window = *windows;
    let ray = cursor_ray(window, camera, transform, &rect);
    let local = window.cursor_position().filter(|p| rect.contains(*p));
    let kind = e.active_kind();
    let tool = e.tool();
    let painting = matches!(kind, Some(DocKind::Prop | DocKind::Tile))
        && matches!(tool, Tool::Paint | Tool::Erase);
    // Hover preview for voxel tools.
    let cell = if painting && !pointer.over_ui && e.editing.is_none() {
        match pointer.viewport_drag {
            Some(ViewportDrag::Paint { axis, layer, .. }) => {
                ray.and_then(|r| plane_cell(&e, r, axis, layer))
            }
            _ => ray.and_then(|r| voxel_target(&e, r, tool == Tool::Erase).map(|(c, _)| c)),
        }
    } else {
        None
    };
    if hover.cell != cell {
        hover.cell = cell;
    }
    // Move handles take priority over every other left-button tool.
    let handle = match (pointer.viewport_drag, local) {
        (None, Some(p)) if !pointer.over_ui && pointer.camera_drag.is_none() => {
            gizmo::hit(&e, camera, transform, p)
        }
        _ => None,
    };
    if gizmo_hover.0 != handle {
        gizmo_hover.0 = handle;
    }
    let target = match (pointer.viewport_drag, local) {
        (None, Some(p)) if handle.is_none() && !pointer.over_ui && !painting => {
            target_at(&e, (camera, transform), p, ray)
        }
        _ => None,
    };
    if target_hover.0 != target {
        target_hover.0 = target;
    }
    let press = mouse.just_pressed(MouseButton::Left)
        && ray.is_some()
        && !pointer.over_ui
        && e.menu.is_none()
        && e.modal.is_none()
        && pointer.camera_drag.is_none();
    if press
        && let Some((handle, part)) = handle
        && let Some(origin) = gizmo::handles(&e)
            .into_iter()
            .find(|(h, ..)| *h == handle)
            .map(|(_, p, _)| p)
        && let Some(grab) =
            ray.and_then(|r| gizmo::grab_point(part, origin, r, transform.forward().into()))
    {
        e.begin_gesture(match handle {
            gizmo::Handle::BoneTip(_) => "Move bone tip",
            gizmo::Handle::BoneJoint(_) => "Move joint",
            gizmo::Handle::Mount(_) => "Move mount",
            gizmo::Handle::Anchor(_) => "Move anchor",
            gizmo::Handle::Socket(_) => "Move socket",
            gizmo::Handle::Attachment(_) => "Move prop",
        });
        pointer.viewport_drag = Some(ViewportDrag::Gizmo(gizmo::GizmoDrag {
            handle,
            part,
            origin,
            grab,
        }));
        return;
    }
    let alt = keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]);
    if press
        && alt
        && matches!(kind, Some(DocKind::Prop | DocKind::Tile))
        && let Some(color) = ray.and_then(|r| pick_color(&e, r))
    {
        e.color = color;
        e.notify(
            NoticeKind::Info,
            format!(
                "Picked color {:.2}, {:.2}, {:.2}",
                color[0], color[1], color[2]
            ),
        );
        e.changed();
        return;
    }
    if press {
        let erase = tool == Tool::Erase;
        if painting {
            if let Some(r) = ray
                && let Some((cell, axis)) = voxel_target(&e, r, erase)
            {
                e.begin_gesture(if erase { "Erase" } else { "Paint" });
                e.view_mut().cursor[1] = if axis == 1 { cell[1] } else { e.cursor()[1] };
                e.gesture_edit(|e| actions::paint_at(e, cell, erase));
                pointer.viewport_drag = Some(ViewportDrag::Paint {
                    axis,
                    layer: cell[axis],
                    erase,
                    last: cell,
                });
            }
        } else if let Some(start) = local {
            pointer.viewport_drag = Some(ViewportDrag::Pick {
                start,
                orbiting: false,
            });
        }
        return;
    }
    match pointer.viewport_drag {
        Some(ViewportDrag::Gizmo(drag)) => {
            if !mouse.pressed(MouseButton::Left) {
                pointer.viewport_drag = None;
                e.end_gesture();
                return;
            }
            let snap = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
            if let Some(point) = ray.and_then(|r| {
                gizmo::grab_point(drag.part, drag.origin, r, transform.forward().into())
            }) {
                let target = drag.origin + (point - drag.grab);
                e.gesture_edit(|e| gizmo::set(e, drag.handle, target, snap));
            }
        }
        Some(ViewportDrag::Paint {
            axis,
            layer,
            erase,
            last,
        }) => {
            if !mouse.pressed(MouseButton::Left) {
                pointer.viewport_drag = None;
                e.end_gesture();
                return;
            }
            if let Some(cell) = ray.and_then(|r| plane_cell(&e, r, axis, layer))
                && cell != last
            {
                e.gesture_edit(|e| actions::paint_at(e, cell, erase));
                pointer.viewport_drag = Some(ViewportDrag::Paint {
                    axis,
                    layer,
                    erase,
                    last: cell,
                });
            }
        }
        Some(ViewportDrag::Pick { start, orbiting }) => {
            let Some(p) = local else {
                if !mouse.pressed(MouseButton::Left) {
                    pointer.viewport_drag = None;
                }
                return;
            };
            if mouse.pressed(MouseButton::Left) {
                let moved = p - start;
                if orbiting || moved.length() > 5. {
                    let delta = pointer.last_cursor.map_or(Vec2::ZERO, |l| p - l);
                    if let Some(cam) = e.cam_mut() {
                        cam.yaw -= delta.x * 0.008;
                        cam.pitch = (cam.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
                    }
                    pointer.viewport_drag = Some(ViewportDrag::Pick {
                        start,
                        orbiting: true,
                    });
                }
                pointer.last_cursor = Some(p);
                return;
            }
            pointer.viewport_drag = None;
            pointer.last_cursor = None;
            if orbiting {
                return;
            }
            click(&mut e, (camera, transform), p, ui_scale.0, ray, &options);
        }
        None => pointer.last_cursor = local,
    }
}

/// What a click at `p` would select, without side effects (also drives the
/// hover highlight). Character mounts report as `Sel::Mount`.
pub fn target_at(
    e: &Editor,
    (camera, t): (&Camera, &GlobalTransform),
    p: Vec2,
    ray: Option<Ray3d>,
) -> Option<Sel> {
    match e.active_kind()? {
        DocKind::Body => pick_mount(e, camera, t, p)
            .map(Sel::Mount)
            .or(pick_bone(e, camera, t, p).map(Sel::Bone)),
        DocKind::Character => pick_mount(e, camera, t, p)
            .filter(|_| e.anim.clip.is_none())
            .map(Sel::Mount)
            .or(pick_bone(e, camera, t, p).map(Sel::Bone)),
        DocKind::World => Some(pick_world(e, camera, t, p, ray?)).filter(|s| *s != Sel::None),
        DocKind::Prop => e.active_prop()?.anchors.iter().find_map(|a| {
            let screen = camera
                .world_to_viewport(t, Vec3::from_array(a.position))
                .ok()?;
            (screen.distance(p) < 14.).then_some(Sel::Anchor(a.id))
        }),
        DocKind::Tile => {
            let tile = e.active_tile()?;
            let view = e.view()?;
            let i = tiles::Instance {
                id: 0,
                tile: tile.id,
                position: [0.; 3],
                rotation: tile.rotations[view.turn % tile.rotations.len()],
            };
            let socket = tile.sockets.iter().enumerate().find_map(|(n, s)| {
                let (pos, _) = tiles::frame(&i, s);
                let screen = camera.world_to_viewport(t, pos).ok()?;
                (screen.distance(p) < 14.).then_some(Sel::Socket(n))
            });
            let collider = (view.tool == Tool::Collider)
                .then_some(ray)
                .flatten()
                .and_then(|ray| {
                    let r = i.transform().rotation.inverse();
                    let (origin, dir) = (r * ray.origin, r * *ray.direction);
                    tile.colliders
                        .iter()
                        .enumerate()
                        .filter_map(|(n, b)| ray_box(origin, dir, b.min, b.max).map(|d| (d, n)))
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .map(|(_, n)| Sel::Collider(n))
                });
            socket.or(collider)
        }
        DocKind::Rules => None,
    }
}

fn click(
    e: &mut Editor,
    view: (&Camera, &GlobalTransform),
    p: Vec2,
    scale: f32,
    ray: Option<Ray3d>,
    options: &LaunchOptions,
) {
    let target = target_at(e, view, p, ray);
    match (e.active_kind(), target) {
        // Clicking a character's mount chooses it for equipping.
        (Some(DocKind::Character), Some(Sel::Mount(mount))) => {
            e.apply(Action::EquipMount(mount), options);
        }
        (Some(DocKind::World), sel) => {
            let sel = sel.unwrap_or(Sel::None);
            e.apply(Action::Select(sel), options);
            if let Sel::PieceSocket(..) = sel {
                // Offer every compatible tile right where the user clicked.
                if world::options(e).is_empty() {
                    e.notify(NoticeKind::Info, "No tile in the library fits this socket");
                } else {
                    e.menu = Some(OpenMenu {
                        menu: Menu::Connect,
                        pos: p / scale + Vec2::new(12., 4.),
                        width: 240.,
                    });
                }
                e.changed();
            }
        }
        // Elsewhere in voxel documents a click moves the cursor.
        (Some(DocKind::Prop | DocKind::Tile), None) => {
            move_cursor(e, ray);
            e.apply(Action::Select(Sel::None), options);
        }
        (_, sel) => e.apply(Action::Select(sel.unwrap_or(Sel::None)), options),
    }
}
