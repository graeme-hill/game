//! Editable values. Numeric [`Field`]s resolve against the active document and
//! selection so a widget only names *what* it edits; dragging, typing and
//! stepping all funnel through [`set`].
use super::*;
use game::tiles;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Field {
    BoneOffset(usize),
    BoneTip(usize),
    BoneRadius,
    MountOffset(usize),
    MountRotation(usize),
    AnchorPosition(usize),
    AnchorRotation(usize),
    AttachOffset(usize),
    AttachRotation(usize),
    AttachScale,
    Cursor(usize),
    Rgb(usize),
    ClipDuration,
    Time,
    KeyRotation(usize),
    KeyOffset(usize),
    Blend,
    SocketPosition(usize),
    ColliderMin(usize),
    ColliderMax(usize),
    Seed,
    WorldSize,
    Houses,
    Plants,
}

pub struct Spec {
    /// Change per stepper click.
    pub step: f32,
    /// Change per dragged logical pixel.
    pub drag: f32,
    pub decimals: usize,
    pub suffix: &'static str,
    pub integer: bool,
}

impl Field {
    pub fn spec(self) -> Spec {
        let (step, drag, decimals, suffix, integer) = match self {
            Field::BoneOffset(_)
            | Field::BoneTip(_)
            | Field::MountOffset(_)
            | Field::AnchorPosition(_)
            | Field::AttachOffset(_) => (0.1, 0.01, 2, "", false),
            Field::BoneRadius => (0.01, 0.005, 2, "", false),
            Field::MountRotation(_)
            | Field::AnchorRotation(_)
            | Field::AttachRotation(_)
            | Field::KeyRotation(_) => (15., 1., 0, "°", false),
            Field::AttachScale => (0.1, 0.01, 2, "×", false),
            Field::Cursor(_) | Field::SocketPosition(_) => (1., 0.15, 0, "", true),
            Field::Rgb(_) => (0.05, 0.004, 2, "", false),
            Field::ClipDuration | Field::Time => (0.1, 0.01, 2, "s", false),
            Field::KeyOffset(_) => (0.025, 0.005, 2, "", false),
            Field::Blend => (0.1, 0.005, 0, "%", false),
            Field::ColliderMin(_) | Field::ColliderMax(_) => (0.1, 0.01, 2, "", false),
            Field::Seed => (1., 0.2, 0, "", true),
            Field::WorldSize | Field::Houses => (1., 0.05, 0, "", true),
            Field::Plants => (5., 0.3, 0, "%", true),
        };
        Spec {
            step,
            drag,
            decimals,
            suffix,
            integer,
        }
    }
    /// A short human name, for undo history.
    pub fn name(self) -> &'static str {
        match self {
            Field::BoneOffset(_) => "joint",
            Field::BoneTip(_) => "tip",
            Field::BoneRadius => "radius",
            Field::MountOffset(_) => "mount offset",
            Field::MountRotation(_) => "mount rotation",
            Field::AnchorPosition(_) => "anchor position",
            Field::AnchorRotation(_) => "anchor rotation",
            Field::AttachOffset(_) => "prop offset",
            Field::AttachRotation(_) => "prop rotation",
            Field::AttachScale => "prop scale",
            Field::Cursor(_) => "cursor",
            Field::Rgb(_) => "color",
            Field::ClipDuration => "clip length",
            Field::Time => "time",
            Field::KeyRotation(_) => "key rotation",
            Field::KeyOffset(_) => "key offset",
            Field::Blend => "blend",
            Field::SocketPosition(_) => "socket position",
            Field::ColliderMin(_) | Field::ColliderMax(_) => "collision box",
            Field::Seed => "seed",
            Field::WorldSize => "world size",
            Field::Houses => "houses",
            Field::Plants => "planting",
        }
    }
    /// Fields that change view or tool state rather than a document.
    pub fn is_view_state(self) -> bool {
        matches!(
            self,
            Field::Cursor(_)
                | Field::Rgb(_)
                | Field::Time
                | Field::Blend
                | Field::Seed
                | Field::WorldSize
                | Field::Houses
                | Field::Plants
        )
    }
    pub fn format(self, value: f32) -> String {
        let spec = self.spec();
        let value = if self == Field::Blend {
            value * 100.
        } else {
            value
        };
        // Avoid "-0.00" for values that round to zero.
        let tiny = 0.5 / 10f32.powi(spec.decimals as i32);
        let value = if value.abs() < tiny { 0. } else { value };
        format!("{:.*}{}", spec.decimals, value, spec.suffix)
    }
}

fn degrees(v: [f32; 3], axis: usize) -> f32 {
    v[axis].to_degrees()
}

/// The current value in display units (degrees for rotations).
pub fn get(e: &Editor, field: Field) -> Option<f32> {
    let sel = e.sel();
    Some(match field {
        Field::BoneOffset(_) | Field::BoneTip(_) | Field::BoneRadius => {
            let Sel::Bone(id) = sel else { return None };
            let b = e.active_body()?.bones.iter().find(|b| b.id == id)?;
            match field {
                Field::BoneOffset(a) => b.offset[a],
                Field::BoneTip(a) => b.tip[a],
                _ => b.radius,
            }
        }
        Field::MountOffset(a) | Field::MountRotation(a) => {
            let Sel::Mount(id) = sel else { return None };
            let m = e.active_body()?.mounts.iter().find(|m| m.id == id)?;
            if matches!(field, Field::MountOffset(_)) {
                m.offset[a]
            } else {
                degrees(m.rotation, a)
            }
        }
        Field::AnchorPosition(a) | Field::AnchorRotation(a) => {
            let Sel::Anchor(id) = sel else { return None };
            let anchor = e.active_prop()?.anchors.iter().find(|v| v.id == id)?;
            if matches!(field, Field::AnchorPosition(_)) {
                anchor.position[a]
            } else {
                degrees(anchor.rotation, a)
            }
        }
        Field::AttachOffset(_) | Field::AttachRotation(_) | Field::AttachScale => {
            let Sel::Attachment(id) = sel else {
                return None;
            };
            let at = e
                .active_character()?
                .attachments
                .iter()
                .find(|v| v.id == id)?;
            match field {
                Field::AttachOffset(a) => at.offset[a],
                Field::AttachRotation(a) => degrees(at.rotation, a),
                _ => at.scale,
            }
        }
        Field::Cursor(a) => e.view()?.cursor[a] as f32,
        Field::Rgb(a) => match (e.active_kind(), sel) {
            (Some(DocKind::Body), Sel::Bone(id)) => {
                e.active_body()?.bones.iter().find(|b| b.id == id)?.color[a]
            }
            _ => e.color[a],
        },
        Field::ClipDuration => e.active_character()?.animations.get(e.anim.clip?)?.duration,
        Field::Time => {
            e.anim.clip?;
            e.anim.time
        }
        Field::KeyRotation(a) => {
            let pose = e.animation_values()?;
            let (x, y, z) = pose.rotation.to_euler(EulerRot::XYZ);
            [x, y, z][a].to_degrees()
        }
        Field::KeyOffset(a) => e.animation_values()?.translation[a],
        Field::Blend => {
            e.anim.blend_target?;
            e.anim.blend
        }
        Field::SocketPosition(a) => {
            let Sel::Socket(i) = sel else { return None };
            e.active_tile()?.sockets.get(i)?.position[a] as f32
        }
        Field::ColliderMin(a) | Field::ColliderMax(a) => {
            let Sel::Collider(i) = sel else { return None };
            let c = e.active_tile()?.colliders.get(i)?;
            if matches!(field, Field::ColliderMin(_)) {
                c.min[a]
            } else {
                c.max[a]
            }
        }
        Field::Seed => e.generator.seed as f32,
        Field::WorldSize => e.generator.size as f32,
        Field::Houses => e.generator.houses as f32,
        Field::Plants => f32::from(e.generator.plants),
    })
}

fn snap(v: f32, decimals: usize) -> f32 {
    let scale = 10f32.powi(decimals as i32);
    (v * scale).round() / scale
}

/// Set a value given in display units. Out-of-range values are clamped.
pub fn set(e: &mut Editor, field: Field, value: f32) -> Result<(), String> {
    if !value.is_finite() {
        return Err("Enter a number".into());
    }
    let spec = field.spec();
    let value = if spec.integer {
        value.round()
    } else if matches!(field, Field::Blend) {
        value
    } else {
        snap(value, spec.decimals.max(2))
    };
    let sel = e.sel();
    let missing = || "Select something to edit".to_string();
    match field {
        Field::BoneOffset(_) | Field::BoneTip(_) | Field::BoneRadius => {
            let Sel::Bone(id) = sel else {
                return Err(missing());
            };
            let b = e
                .active_body_mut()
                .and_then(|b| b.bones.iter_mut().find(|b| b.id == id))
                .ok_or_else(missing)?;
            match field {
                Field::BoneOffset(a) => b.offset[a] = value.clamp(-50., 50.),
                Field::BoneTip(a) => b.tip[a] = value.clamp(-50., 50.),
                _ => b.radius = value.clamp(0.03, 1.5),
            }
        }
        Field::MountOffset(a) | Field::MountRotation(a) => {
            let Sel::Mount(id) = sel else {
                return Err(missing());
            };
            let m = e
                .active_body_mut()
                .and_then(|b| b.mounts.iter_mut().find(|m| m.id == id))
                .ok_or_else(missing)?;
            if matches!(field, Field::MountOffset(_)) {
                m.offset[a] = value.clamp(-50., 50.);
            } else {
                m.rotation[a] = wrap_degrees(value).to_radians();
            }
        }
        Field::AnchorPosition(a) | Field::AnchorRotation(a) => {
            let Sel::Anchor(id) = sel else {
                return Err(missing());
            };
            let anchor = e
                .active_prop_mut()
                .and_then(|p| p.anchors.iter_mut().find(|v| v.id == id))
                .ok_or_else(missing)?;
            if matches!(field, Field::AnchorPosition(_)) {
                anchor.position[a] = value.clamp(-50., 50.);
            } else {
                anchor.rotation[a] = wrap_degrees(value).to_radians();
            }
        }
        Field::AttachOffset(_) | Field::AttachRotation(_) | Field::AttachScale => {
            let Sel::Attachment(id) = sel else {
                return Err(missing());
            };
            let at = e
                .active_character_mut()
                .and_then(|c| c.attachments.iter_mut().find(|v| v.id == id))
                .ok_or_else(missing)?;
            match field {
                Field::AttachOffset(a) => at.offset[a] = value.clamp(-50., 50.),
                Field::AttachRotation(a) => at.rotation[a] = wrap_degrees(value).to_radians(),
                _ => at.scale = value.clamp(0.1, 4.),
            }
        }
        Field::Cursor(a) => {
            let (lo, hi) = if e.active_kind() == Some(DocKind::Prop) {
                (0, 32 - e.brush as i32)
            } else {
                (-256, 256)
            };
            e.active.ok_or("Open a document")?;
            e.view_mut().cursor[a] = (value as i32).clamp(lo, hi);
        }
        Field::Rgb(a) => {
            let value = value.clamp(0., 1.);
            if let (Some(DocKind::Body), Sel::Bone(id)) = (e.active_kind(), sel) {
                let b = e
                    .active_body_mut()
                    .and_then(|b| b.bones.iter_mut().find(|b| b.id == id))
                    .ok_or_else(missing)?;
                b.color[a] = value;
                e.color = b.color;
            } else {
                e.color[a] = value;
            }
        }
        Field::ClipDuration | Field::Time => {
            animation::set_time_field(e, field, value)?;
        }
        Field::KeyRotation(a) => animation::set_key(e, a, value.to_radians(), false)?,
        Field::KeyOffset(a) => animation::set_key(e, a, value, true)?,
        Field::Blend => e.anim.blend = value.clamp(0., 1.),
        Field::SocketPosition(a) => {
            let Sel::Socket(i) = sel else {
                return Err(missing());
            };
            let s = e
                .active_tile_mut()
                .and_then(|t| t.sockets.get_mut(i))
                .ok_or_else(missing)?;
            s.position[a] = (value as i32).clamp(-512, 512);
        }
        Field::ColliderMin(a) | Field::ColliderMax(a) => {
            let Sel::Collider(i) = sel else {
                return Err(missing());
            };
            let c = e
                .active_tile_mut()
                .and_then(|t| t.colliders.get_mut(i))
                .ok_or_else(missing)?;
            // Keep boxes non-degenerate while either face is dragged.
            if matches!(field, Field::ColliderMin(_)) {
                c.min[a] = value.min(c.max[a] - tiles::UNIT);
            } else {
                c.max[a] = value.max(c.min[a] + tiles::UNIT);
            }
        }
        Field::Seed => e.generator.seed = value.max(0.) as u64,
        Field::WorldSize => e.generator.size = (value as usize).clamp(5, 11),
        Field::Houses => e.generator.houses = (value as usize).clamp(0, 30),
        Field::Plants => e.generator.plants = value.clamp(0., 100.) as u8,
    }
    Ok(())
}

fn wrap_degrees(v: f32) -> f32 {
    (v + 180.).rem_euclid(360.) - 180.
}

/// Text-editable values. Names act on the current selection.
#[derive(Clone, Debug, PartialEq)]
pub enum TextTarget {
    DocName(DocRef),
    BoneName,
    MountName,
    AnchorName,
    ClipName,
    SocketName,
    SocketKind,
    SocketProfile,
    TileCategory,
    NewRuleType,
    Filter,
    Path,
    Value(Field),
}

impl TextTarget {
    pub fn limit(&self) -> usize {
        match self {
            TextTarget::Path => 240,
            TextTarget::Value(_) => 16,
            _ => 32,
        }
    }
}

pub fn text_value(e: &Editor, target: &TextTarget) -> String {
    let sel = e.sel();
    let name = || -> Option<String> {
        Some(match target {
            TextTarget::DocName(doc) => e.doc_name(*doc),
            TextTarget::BoneName => {
                let Sel::Bone(id) = sel else { return None };
                e.preview_body()?
                    .bones
                    .iter()
                    .find(|b| b.id == id)?
                    .name
                    .clone()
            }
            TextTarget::MountName => {
                let Sel::Mount(id) = sel else { return None };
                e.active_body()?
                    .mounts
                    .iter()
                    .find(|m| m.id == id)?
                    .name
                    .clone()
            }
            TextTarget::AnchorName => {
                let Sel::Anchor(id) = sel else { return None };
                e.active_prop()?
                    .anchors
                    .iter()
                    .find(|a| a.id == id)?
                    .name
                    .clone()
            }
            TextTarget::ClipName => e
                .active_character()?
                .animations
                .get(e.anim.clip?)?
                .name
                .clone(),
            TextTarget::SocketName | TextTarget::SocketKind | TextTarget::SocketProfile => {
                let Sel::Socket(i) = sel else { return None };
                let s = e.active_tile()?.sockets.get(i)?;
                match target {
                    TextTarget::SocketName => s.name.clone(),
                    TextTarget::SocketKind => s.kind.clone(),
                    _ => s.profile.clone(),
                }
            }
            TextTarget::TileCategory => e.active_tile()?.category.clone(),
            TextTarget::NewRuleType => String::new(),
            TextTarget::Filter => e.filter.clone(),
            TextTarget::Path => e.workspace_dir.display().to_string(),
            TextTarget::Value(field) => {
                let value = get(e, *field)?;
                let shown = if *field == Field::Blend {
                    value * 100.
                } else {
                    value
                };
                let text = format!("{:.*}", field.spec().decimals.max(2), shown);
                if text.contains('.') {
                    text.trim_end_matches('0').trim_end_matches('.').to_string()
                } else {
                    text
                }
            }
        })
    };
    name().unwrap_or_default()
}

pub fn commit_text(e: &mut Editor, target: &TextTarget, text: &str) -> Result<(), String> {
    let sel = e.sel();
    if let TextTarget::Value(field) = target {
        let value: f32 = text
            .trim_end_matches(['°', 's', '%', '×'])
            .trim()
            .parse()
            .map_err(|_| format!("'{text}' is not a number"))?;
        let value = if *field == Field::Blend {
            value / 100.
        } else {
            value
        };
        return set(e, *field, value);
    }
    match target {
        TextTarget::Filter => {
            e.filter = text.into();
            return Ok(());
        }
        TextTarget::NewRuleType => {
            if text.is_empty() {
                return Ok(());
            }
            if actions::socket_kinds(&e.library).iter().any(|k| k == text) {
                return Err(format!("Socket type {text} already exists"));
            }
            e.library
                .socket_rules
                .pairs
                .push([text.into(), text.into()]);
            e.rule_focus = Some((text.into(), text.into()));
            return Ok(());
        }
        _ => {}
    }
    if text.is_empty() {
        return Err("Name cannot be empty".into());
    }
    let text: String = text.chars().take(target.limit()).collect();
    let slot: Option<&mut String> = match target {
        TextTarget::DocName(doc) => {
            let l = &mut e.library;
            let id = doc.id;
            match doc.kind {
                DocKind::Body => l
                    .bodies
                    .iter_mut()
                    .find(|v| v.id == id)
                    .map(|v| &mut v.name),
                DocKind::Prop => l.props.iter_mut().find(|v| v.id == id).map(|v| &mut v.name),
                DocKind::Character => l
                    .characters
                    .iter_mut()
                    .find(|v| v.id == id)
                    .map(|v| &mut v.name),
                DocKind::Tile => l.tiles.iter_mut().find(|v| v.id == id).map(|v| &mut v.name),
                DocKind::World => l
                    .worlds
                    .iter_mut()
                    .find(|v| v.id == id)
                    .map(|v| &mut v.name),
                DocKind::Rules => return Err("Socket rules cannot be renamed".into()),
            }
        }
        TextTarget::BoneName => {
            let Sel::Bone(id) = sel else {
                return Err("Select a bone".into());
            };
            e.active_body_mut()
                .and_then(|b| b.bones.iter_mut().find(|b| b.id == id))
                .map(|b| &mut b.name)
        }
        TextTarget::MountName => {
            let Sel::Mount(id) = sel else {
                return Err("Select a mount".into());
            };
            e.active_body_mut()
                .and_then(|b| b.mounts.iter_mut().find(|m| m.id == id))
                .map(|m| &mut m.name)
        }
        TextTarget::AnchorName => {
            let Sel::Anchor(id) = sel else {
                return Err("Select an anchor".into());
            };
            e.active_prop_mut()
                .and_then(|p| p.anchors.iter_mut().find(|a| a.id == id))
                .map(|a| &mut a.name)
        }
        TextTarget::ClipName => {
            let clip = e.anim.clip.ok_or("Select an animation")?;
            if e.active_character().is_some_and(|c| {
                c.animations
                    .iter()
                    .enumerate()
                    .any(|(i, a)| i != clip && a.name.eq_ignore_ascii_case(&text))
            }) {
                return Err(format!("An animation named {text} already exists"));
            }
            e.active_character_mut()
                .and_then(|c| c.animations.get_mut(clip))
                .map(|a| &mut a.name)
        }
        TextTarget::SocketName | TextTarget::SocketKind | TextTarget::SocketProfile => {
            let Sel::Socket(i) = sel else {
                return Err("Select a socket".into());
            };
            e.active_tile_mut()
                .and_then(|t| t.sockets.get_mut(i))
                .map(|s| match target {
                    TextTarget::SocketName => &mut s.name,
                    TextTarget::SocketKind => &mut s.kind,
                    _ => &mut s.profile,
                })
        }
        TextTarget::TileCategory => e.active_tile_mut().map(|t| &mut t.category),
        _ => None,
    };
    *slot.ok_or("Nothing to rename")? = text;
    Ok(())
}

/// sRGB (0..1) to hue (degrees), saturation and value.
pub fn rgb_to_hsv(c: [f32; 3]) -> (f32, f32, f32) {
    let max = c[0].max(c[1]).max(c[2]);
    let min = c[0].min(c[1]).min(c[2]);
    let delta = max - min;
    let hue = if delta < 1e-6 {
        0.
    } else if max == c[0] {
        60. * ((c[1] - c[2]) / delta).rem_euclid(6.)
    } else if max == c[1] {
        60. * ((c[2] - c[0]) / delta + 2.)
    } else {
        60. * ((c[0] - c[1]) / delta + 4.)
    };
    let saturation = if max < 1e-6 { 0. } else { delta / max };
    (hue, saturation, max)
}

pub fn hsv_to_rgb(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let c = value * saturation;
    let h = hue.rem_euclid(360.) / 60.;
    let x = c * (1. - (h.rem_euclid(2.) - 1.).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.),
        1 => (x, c, 0.),
        2 => (0., c, x),
        3 => (0., x, c),
        4 => (x, 0., c),
        _ => (c, 0., x),
    };
    let m = value - c;
    [r + m, g + m, b + m].map(|v| v.clamp(0., 1.))
}

/// The color the picker edits: the selected bone's, or the voxel brush's.
pub fn current_color(e: &Editor) -> [f32; 3] {
    [0, 1, 2].map(|axis| get(e, Field::Rgb(axis)).unwrap_or(e.color[axis]))
}

/// Set the picker's color through the RGB fields (so bone colors are document
/// edits and brush colors are tool state).
pub fn set_color(e: &mut Editor, rgb: [f32; 3]) -> Result<(), String> {
    for (axis, value) in rgb.into_iter().enumerate() {
        set(e, Field::Rgb(axis), value)?;
    }
    Ok(())
}
