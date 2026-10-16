//! Move handles in the viewport for the selected bone, mount, anchor or
//! socket. Dragging edits the same [`Field`]s as the Inspector, so clamping,
//! snapping and undo behave identically.
use super::*;
use game::tiles;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Handle {
    BoneTip(u32),
    BoneJoint(u32),
    Mount(u32),
    Anchor(u32),
    Socket(usize),
    /// An equipped prop: moves its offset in the mount's frame.
    Attachment(u32),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    /// Move in the view plane.
    Free,
    /// Move along a world axis.
    Axis(usize),
}

/// The handle part under the pointer, for highlighting.
#[derive(Resource, Default)]
pub struct GizmoHover(pub Option<(Handle, Part)>);

/// A handle drag in progress: where it started and where it was grabbed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoDrag {
    pub handle: Handle,
    pub part: Part,
    pub origin: Vec3,
    pub grab: Vec3,
}

fn tile_rotation(e: &Editor) -> Option<Quat> {
    let t = e.active_tile()?;
    let turn = e.view()?.turn;
    Some(Quat::from_array(t.rotations[turn % t.rotations.len()]))
}

/// Handles for the current selection: (handle, world position, has axes).
pub fn handles(e: &Editor) -> Vec<(Handle, Vec3, bool)> {
    let found = || -> Option<Vec<(Handle, Vec3, bool)>> {
        Some(match (e.active_kind()?, e.sel()) {
            (DocKind::Body, Sel::Bone(id)) => {
                let (start, end) = e.active_body()?.bone_segment(id)?;
                vec![
                    (Handle::BoneTip(id), Vec3::from_array(end), true),
                    (Handle::BoneJoint(id), Vec3::from_array(start), false),
                ]
            }
            (DocKind::Body, Sel::Mount(id)) => {
                let body = e.active_body()?;
                let m = body.mounts.iter().find(|m| m.id == id)?;
                let (_, end) = body.bone_segment(m.bone)?;
                vec![(
                    Handle::Mount(id),
                    Vec3::from_array(end) + Vec3::from_array(m.offset),
                    true,
                )]
            }
            (DocKind::Prop, Sel::Anchor(id)) => {
                let a = e.active_prop()?.anchors.iter().find(|a| a.id == id)?;
                vec![(Handle::Anchor(id), Vec3::from_array(a.position), true)]
            }
            (DocKind::Tile, Sel::Socket(i)) => {
                let s = e.active_tile()?.sockets.get(i)?;
                let p = Vec3::from_array(s.position.map(|v| v as f32 * tiles::UNIT));
                vec![(Handle::Socket(i), tile_rotation(e)? * p, true)]
            }
            (DocKind::Character, Sel::Attachment(id)) => {
                let (point, rotation, offset) = attachment_frame(e, id)?;
                vec![(Handle::Attachment(id), point + rotation * offset, true)]
            }
            _ => return None,
        })
    };
    found().unwrap_or_default()
}

/// Where an attachment's mount sits (posed), the mount's rotation, and the
/// attachment's offset in that frame.
fn attachment_frame(e: &Editor, id: u32) -> Option<(Vec3, Quat, Vec3)> {
    let a = e
        .active_character()?
        .attachments
        .iter()
        .find(|a| a.id == id)?;
    let body = e.preview_body()?;
    let mount = body.mounts.iter().find(|m| m.id == a.mount)?;
    let index = body.bones.iter().position(|b| b.id == mount.bone)?;
    let frame = *viewport::preview_frames(e)?.get(index)?;
    let rotation = frame.rotation * game::animation::euler(mount.rotation);
    let point = frame.end + frame.rotation * Vec3::from_array(mount.offset);
    Some((point, rotation, Vec3::from_array(a.offset)))
}

/// World-space arrow length that stays a similar size on screen.
pub fn arrow_length(e: &Editor) -> f32 {
    e.cam().distance * 0.14
}

fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = if ab.length_squared() > 0. {
        ((p - a).dot(ab) / ab.length_squared()).clamp(0., 1.)
    } else {
        0.
    };
    p.distance(a + ab * t)
}

/// The handle part under the cursor (window coordinates).
pub fn hit(
    e: &Editor,
    camera: &Camera,
    t: &GlobalTransform,
    cursor: Vec2,
) -> Option<(Handle, Part)> {
    let length = arrow_length(e);
    let mut best: Option<((Handle, Part), f32)> = None;
    for (handle, p, axes) in handles(e) {
        let Ok(center) = camera.world_to_viewport(t, p) else {
            continue;
        };
        let d = center.distance(cursor);
        if d < 11. && best.is_none_or(|(_, b)| d < b) {
            best = Some(((handle, Part::Free), d));
        }
        if !axes {
            continue;
        }
        for axis in 0..3 {
            let Ok(end) = camera.world_to_viewport(t, p + Vec3::AXES[axis] * length) else {
                continue;
            };
            // Ignore the part of the arrow hidden behind the center handle.
            let d = segment_distance(cursor, center.lerp(end, 0.2), end);
            if d < 7. && best.is_none_or(|(_, b)| d + 3. < b) {
                best = Some(((handle, Part::Axis(axis)), d));
            }
        }
    }
    best.map(|(h, _)| h)
}

/// The point a ray grabs on a handle part: the closest point on an axis line,
/// or the intersection with the view-facing plane through the handle.
pub fn grab_point(part: Part, origin: Vec3, ray: Ray3d, view_forward: Vec3) -> Option<Vec3> {
    let dir = *ray.direction;
    match part {
        Part::Free => {
            let denom = dir.dot(view_forward);
            if denom.abs() < 1e-5 {
                return None;
            }
            let s = (origin - ray.origin).dot(view_forward) / denom;
            (s > 0.).then(|| ray.origin + dir * s)
        }
        Part::Axis(axis) => {
            // Closest point between the axis line and the ray.
            let a = Vec3::AXES[axis];
            let w = origin - ray.origin;
            let b = a.dot(dir);
            let denom = 1. - b * b;
            if denom.abs() < 1e-4 {
                return None;
            }
            let s = (b * dir.dot(w) - a.dot(w)) / denom;
            Some(origin + a * s)
        }
    }
}

/// Move a handle to a world position by writing the Inspector's fields.
pub fn set(e: &mut Editor, handle: Handle, world: Vec3, snap: bool) -> Result<(), String> {
    let snapped = |v: f32| if snap { (v * 10.).round() / 10. } else { v };
    let write = |e: &mut Editor, field: fn(usize) -> Field, value: Vec3| -> Result<(), String> {
        for axis in 0..3 {
            fields::set(e, field(axis), snapped(value[axis]))?;
        }
        Ok(())
    };
    match handle {
        Handle::BoneTip(id) => {
            let (start, _) = e
                .active_body()
                .and_then(|b| b.bone_segment(id))
                .ok_or("Select a bone")?;
            write(e, Field::BoneTip, world - Vec3::from_array(start))
        }
        Handle::BoneJoint(id) => {
            let body = e.active_body().ok_or("Open a body")?;
            let parent_end = body
                .bones
                .iter()
                .find(|b| b.id == id)
                .and_then(|b| b.parent)
                .and_then(|p| body.bone_segment(p))
                .map_or(Vec3::ZERO, |(_, end)| Vec3::from_array(end));
            write(e, Field::BoneOffset, world - parent_end)
        }
        Handle::Mount(id) => {
            let body = e.active_body().ok_or("Open a body")?;
            let m = body
                .mounts
                .iter()
                .find(|m| m.id == id)
                .ok_or("Select a mount")?;
            let (_, end) = body.bone_segment(m.bone).ok_or("Missing bone")?;
            write(e, Field::MountOffset, world - Vec3::from_array(end))
        }
        Handle::Anchor(_) => write(e, Field::AnchorPosition, world),
        Handle::Attachment(id) => {
            let (point, rotation, _) = attachment_frame(e, id).ok_or("Select an equipped prop")?;
            write(e, Field::AttachOffset, rotation.inverse() * (world - point))
        }
        Handle::Socket(_) => {
            let local = tile_rotation(e).ok_or("Open a tile")?.inverse() * world / tiles::UNIT;
            for axis in 0..3 {
                fields::set(e, Field::SocketPosition(axis), local[axis].round())?;
            }
            Ok(())
        }
    }
}

pub fn draw(
    e: &Editor,
    hover: Option<(Handle, Part)>,
    active: Option<GizmoDrag>,
    g: &mut Gizmos<viewport::OverlayGuides>,
) {
    let theme = ui::theme::Palette::get();
    let length = arrow_length(e);
    let lit = |handle: Handle, part: Part| {
        active.is_some_and(|d| d.handle == handle && d.part == part)
            || (active.is_none() && hover == Some((handle, part)))
    };
    for (handle, p, axes) in handles(e) {
        if axes {
            for axis in 0..3 {
                let color = if lit(handle, Part::Axis(axis)) {
                    theme.warning
                } else {
                    theme.axis(axis)
                };
                g.arrow(p, p + Vec3::AXES[axis] * length, color)
                    .with_tip_length(length * 0.22);
            }
        }
        let color = if lit(handle, Part::Free) {
            theme.warning
        } else if axes {
            Color::WHITE
        } else {
            theme.accent_text
        };
        g.sphere(
            Isometry3d::from_translation(p),
            length * if axes { 0.07 } else { 0.055 },
            color,
        );
    }
}
