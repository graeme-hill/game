use crate::{editor::*, launch::LaunchOptions};
use bevy::{
    camera::{Viewport, visibility::RenderLayers},
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
};
use game::{character_render::CharacterRenderer, voxel};
#[derive(Component)]
pub struct Preview;
#[derive(Component)]
pub struct PreviewCamera;
#[derive(Default, Reflect, GizmoConfigGroup)]
pub struct OverlayGuides;

pub fn setup(mut commands: Commands, mut gizmo_config: ResMut<GizmoConfigStore>) {
    // Armature/anchor editing guides must remain visible through the SDF surface.
    gizmo_config.config_mut::<OverlayGuides>().0.depth_bias = -1.0;
    commands.spawn((
        Camera3d::default(),
        Camera {
            viewport: Some(Viewport {
                physical_position: UVec2::new(300, 48),
                physical_size: UVec2::new(975, 625),
                ..default()
            }),
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

pub fn responsive_layout(
    windows: Single<&Window>,
    mut ui_scale: ResMut<UiScale>,
    mut cameras: Query<&mut Camera, With<PreviewCamera>>,
) {
    let width = windows.resolution.physical_width().max(1);
    let height = windows.resolution.physical_height().max(1);
    // The editor has a deliberate desktop layout at 1280×720.  It scales down
    // for smaller windows, but never inflates into a toy-like oversized UI.
    let scale = (width as f32 / 1280.0)
        .min(height as f32 / 720.0)
        .clamp(0.70, 1.15);
    if (ui_scale.0 - scale).abs() > 0.001 {
        ui_scale.0 = scale;
    }
    let left = (235.0 * scale).round() as u32;
    let top = (48.0 * scale).round() as u32;
    let right = (290.0 * scale).round() as u32;
    let bottom = (58.0 * scale).round() as u32;
    let viewport = Viewport {
        physical_position: UVec2::new(left, top),
        physical_size: UVec2::new(
            width.saturating_sub(left + right).max(1),
            height.saturating_sub(top + bottom).max(1),
        ),
        ..default()
    };
    for mut camera in &mut cameras {
        camera.viewport = Some(viewport.clone());
    }
}
pub fn rebuild(
    mut renderer: CharacterRenderer,
    e: Res<Editor>,
    old: Query<Entity, With<Preview>>,
    mut last: Local<u64>,
) {
    if *last == e.revision {
        return;
    }
    *last = e.revision;
    for entity in &old {
        renderer.commands.entity(entity).despawn();
    }
    let entities = match e.mode {
        Mode::Menu => vec![],
        Mode::Tiles | Mode::Worlds => crate::editor_world::render(&e, &mut renderer),
        Mode::Props => e
            .prop()
            .and_then(|prop| renderer.prop(prop, Transform::default()))
            .into_iter()
            .collect(),
        Mode::Characters => e
            .character()
            .map(|character| renderer.character(&e.library, character, Vec3::ZERO))
            .unwrap_or_default(),
        Mode::Bodies => e
            .preview_body()
            .map(|body| vec![renderer.body(body, Vec3::ZERO)])
            .unwrap_or_default(),
    };
    for entity in entities {
        renderer.commands.entity(entity).insert(Preview);
    }
}
fn target(e: &Editor) -> Vec3 {
    if matches!(e.mode, Mode::Tiles | Mode::Worlds) {
        let (a, b) = crate::editor_world::preview_bounds(e);
        return (a + b) * 0.5;
    }
    if e.mode == Mode::Props {
        return Vec3::new(1.6, 0.7, 1.6);
    }
    if let Some(body) = e.preview_body() {
        let mut min = Vec3::splat(f32::INFINITY);
        let mut max = Vec3::splat(f32::NEG_INFINITY);
        for b in &body.bones {
            if let Some((a, z)) = body.bone_segment(b.id) {
                min = min.min(Vec3::from_array(a)).min(Vec3::from_array(z));
                max = max.max(Vec3::from_array(a)).max(Vec3::from_array(z));
            }
        }
        if min.is_finite() {
            return (min + max) * 0.5;
        }
    }
    Vec3::Y
}
pub fn camera(e: Res<Editor>, mut cameras: Query<&mut Transform, With<PreviewCamera>>) {
    if !e.is_changed() {
        return;
    }
    let center = target(&e) + e.pan;
    let horizontal = e.pitch.cos();
    let offset = Vec3::new(
        e.yaw.sin() * horizontal,
        e.pitch.sin(),
        e.yaw.cos() * horizontal,
    ) * e.distance;
    for mut t in &mut cameras {
        *t = Transform::from_translation(center + offset).looking_at(center, Vec3::Y);
    }
}
pub fn camera_input(
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Single<&Window>,
    mut e: ResMut<Editor>,
) {
    let delta: Vec2 = motion.read().map(|m| m.delta).sum();
    let scroll: f32 = wheel.read().map(|w| w.y).sum();
    if e.mode == Mode::Menu || e.naming.is_some() {
        return;
    }
    let scale = (windows.resolution.physical_width() as f32 / 1280.0)
        .min(windows.resolution.physical_height() as f32 / 720.0)
        .clamp(0.70, 1.15);
    let left = 235.0 * scale;
    let top = 48.0 * scale;
    let bottom = windows.resolution.physical_height() as f32 - 58.0 * scale;
    if windows
        .cursor_position()
        .is_some_and(|p| p.x > left && p.y > top && p.y < bottom)
    {
        if mouse.pressed(MouseButton::Right) && delta != Vec2::ZERO {
            e.yaw -= delta.x * 0.01;
            e.pitch = (e.pitch - delta.y * 0.01).clamp(-1.25, 1.25);
            e.changed();
        }
        if mouse.pressed(MouseButton::Middle) && delta != Vec2::ZERO {
            let right = Vec3::new(e.yaw.cos(), 0., -e.yaw.sin());
            let forward = Vec3::new(e.yaw.sin(), 0., e.yaw.cos());
            let distance = e.distance;
            e.pan += right * (-delta.x * distance * 0.002) + forward * (delta.y * distance * 0.002);
            e.changed();
        }
        if scroll != 0. {
            e.distance = (e.distance
                - scroll
                    * if matches!(e.mode, Mode::Tiles | Mode::Worlds) {
                        2.
                    } else {
                        0.3
                    })
            .clamp(
                2.,
                if matches!(e.mode, Mode::Tiles | Mode::Worlds) {
                    200.
                } else {
                    14.
                },
            );
            e.changed();
        }
    }
}
pub fn guides(e: Res<Editor>, mut g: Gizmos, mut overlay: Gizmos<OverlayGuides>) {
    if matches!(e.mode, Mode::Tiles | Mode::Worlds | Mode::Menu)
        || !e.guides
        || (e.mode == Mode::Characters && e.animation.enabled)
    {
        return;
    }
    let grid = Color::srgb(0.20, 0.22, 0.24);
    let selected = Color::srgb(0.96, 0.97, 0.98);
    if e.mode == Mode::Props {
        let y = e.cursor[1] as f32 * voxel::VOXEL_SIZE;
        for i in 0..=32 {
            let p = i as f32 * voxel::VOXEL_SIZE;
            g.line(Vec3::new(p, y, 0.), Vec3::new(p, y, 3.2), grid);
            g.line(Vec3::new(0., y, p), Vec3::new(3.2, y, p), grid);
        }
        let size = e.brush as f32 * voxel::VOXEL_SIZE;
        let center = Vec3::from_array(e.cursor.map(|v| v as f32 * voxel::VOXEL_SIZE))
            + Vec3::splat(size * 0.5);
        if !e.prop_anchors {
            overlay.cube(
                Transform::from_translation(center).with_scale(Vec3::splat(size)),
                selected,
            );
        }
        if let Some(prop) = e.prop() {
            for (i, a) in prop.anchors.iter().enumerate() {
                cross(
                    &mut overlay,
                    Vec3::from_array(a.position),
                    if i == e.anchor {
                        selected
                    } else {
                        Color::WHITE
                    },
                );
            }
        }
    } else {
        for i in -4..=4 {
            let f = i as f32 * 0.5;
            g.line(Vec3::new(f, 0., -2.), Vec3::new(f, 0., 2.), grid);
            g.line(Vec3::new(-2., 0., f), Vec3::new(2., 0., f), grid);
        }
        if let Some(body) = e.preview_body() {
            if e.mode == Mode::Bodies {
                for (i, b) in body.bones.iter().enumerate() {
                    if let Some((a, z)) = body.bone_segment(b.id) {
                        let color = if i == e.bone {
                            selected
                        } else {
                            Color::srgb(0.93, 0.94, 0.96)
                        };
                        overlay.line(Vec3::from_array(a), Vec3::from_array(z), color);
                        cross(&mut overlay, Vec3::from_array(z), color);
                    }
                }
            }
            for (i, m) in body.mounts.iter().enumerate() {
                if let Some((_, end)) = body.bone_segment(m.bone) {
                    cross(
                        &mut overlay,
                        Vec3::from_array(end) + Vec3::from_array(m.offset),
                        if i == e.mount { selected } else { Color::WHITE },
                    );
                }
            }
        }
    }
}
fn cross(g: &mut Gizmos<OverlayGuides>, p: Vec3, c: Color) {
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        g.line(p - axis * 0.10, p + axis * 0.10, c);
    }
}
fn cursor_in_preview(window: &Window, camera: &Camera) -> Option<Vec2> {
    let cursor = window.cursor_position()?;
    let viewport = camera.viewport.as_ref()?;
    let origin = viewport.physical_position.as_vec2();
    let size = viewport.physical_size.as_vec2();
    (cursor.x >= origin.x
        && cursor.x <= origin.x + size.x
        && cursor.y >= origin.y
        && cursor.y <= origin.y + size.y)
        .then_some(cursor)
}
pub fn paint_viewport(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Single<&Window>,
    cameras: Single<(&Camera, &GlobalTransform), With<PreviewCamera>>,
    mut e: ResMut<Editor>,
    options: Res<LaunchOptions>,
) {
    if !mouse.just_pressed(MouseButton::Left)
        || e.mode != Mode::Props
        || e.prop_anchors
        || e.naming.is_some()
    {
        return;
    }
    let (camera, t) = *cameras;
    let Some(cursor) = cursor_in_preview(&windows, camera) else {
        return;
    };
    let Ok(ray) = camera.viewport_to_world(t, cursor) else {
        return;
    };
    let Some(prop) = e.prop() else { return };
    let hit = voxel::raycast(prop, ray.origin, *ray.direction);
    let cell = if let Some(hit) = hit {
        let mut cell = hit.cell;
        if !e.erase {
            for (axis, v) in cell.iter_mut().enumerate() {
                *v += hit.normal[axis];
            }
        }
        cell
    } else {
        if ray.direction.y.abs() < 0.0001 {
            return;
        }
        let y = e.cursor[1] as f32 * voxel::VOXEL_SIZE;
        let distance = (y - ray.origin.y) / ray.direction.y;
        if distance < 0. {
            return;
        }
        let p = ray.origin + *ray.direction * distance;
        (p / voxel::VOXEL_SIZE).floor().as_ivec3().to_array()
    };
    if cell.iter().any(|v| *v < 0 || *v >= 32) {
        return;
    }
    e.cursor = cell.map(|v| (v / e.brush as i32 * e.brush as i32).clamp(0, 32 - e.brush as i32));
    let erase = e.erase;
    e.apply(Action::Paint(erase), &options);
}

/// Select the nearest visible bone when the preview is clicked. This keeps
/// character editing tactile: the tree and the model stay in sync.
pub fn select_bone_viewport(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Single<&Window>,
    cameras: Single<(&Camera, &GlobalTransform), With<PreviewCamera>>,
    mut e: ResMut<Editor>,
    options: Res<LaunchOptions>,
) {
    if !mouse.just_pressed(MouseButton::Left)
        || matches!(e.mode, Mode::Menu | Mode::Tiles | Mode::Worlds)
        || e.naming.is_some()
        || e.mode == Mode::Props
        || (e.mode == Mode::Characters && e.animation.enabled)
    {
        return;
    }
    let (camera, transform) = *cameras;
    let Some(cursor) = cursor_in_preview(&windows, camera) else {
        return;
    };
    let Some(body) = e.preview_body() else { return };
    let mut best = None;
    for (index, bone) in body.bones.iter().enumerate() {
        let Some((start, end)) = body.bone_segment(bone.id) else {
            continue;
        };
        let (Ok(a), Ok(b)) = (
            camera.world_to_viewport(transform, Vec3::from_array(start)),
            camera.world_to_viewport(transform, Vec3::from_array(end)),
        ) else {
            continue;
        };
        let ab = b - a;
        let t = if ab.length_squared() > 0. {
            ((cursor - a).dot(ab) / ab.length_squared()).clamp(0., 1.)
        } else {
            0.
        };
        let distance = cursor.distance(a.lerp(b, t));
        if distance < best.map_or(28., |(_, d): (usize, f32)| d) {
            best = Some((index, distance));
        }
    }
    if let Some((index, _)) = best {
        e.apply(Action::SelectBone(index), &options);
    }
}
