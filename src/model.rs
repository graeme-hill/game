//! Editable source assets; playback state is separate from saved animation clips.
use bevy::prelude::{EulerRot, Quat, Transform, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const VOXEL_UNIT: f32 = 0.1;
pub const GRID_SIZE: i32 = 32;
pub const MAX_BONES: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Library {
    pub version: u32,
    pub bodies: Vec<Body>,
    pub props: Vec<Prop>,
    pub characters: Vec<Character>,
}
impl Default for Library {
    fn default() -> Self {
        Self {
            version: 1,
            bodies: vec![],
            props: vec![],
            characters: vec![],
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Body {
    pub id: u32,
    pub name: String,
    pub bones: Vec<Bone>,
    pub mounts: Vec<Mount>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Bone {
    pub id: u32,
    pub name: String,
    pub parent: Option<u32>,
    pub offset: [f32; 3],
    pub tip: [f32; 3],
    pub radius: f32,
    pub shape: BodyShape,
    pub color: [f32; 3],
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum BodyShape {
    Sphere,
    Capsule,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mount {
    pub id: u32,
    pub name: String,
    pub bone: u32,
    pub offset: [f32; 3],
    pub rotation: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Prop {
    pub id: u32,
    pub name: String,
    pub blocks: Vec<VoxelBlock>,
    pub anchors: Vec<Anchor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VoxelBlock {
    pub min: [i32; 3],
    pub size: u32,
    pub color: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Anchor {
    pub id: u32,
    pub name: String,
    pub position: [f32; 3],
    pub rotation: [f32; 3],
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Character {
    pub id: u32,
    pub name: String,
    pub body: u32,
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub animations: Vec<crate::animation::AnimationClip>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    pub id: u32,
    pub prop: u32,
    pub anchor: u32,
    pub mount: u32,
    pub offset: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: f32,
}
fn finite(v: [f32; 3]) -> bool {
    v.iter().all(|x| x.is_finite() && x.abs() <= 1000.)
}
fn color_valid(v: [f32; 3]) -> bool {
    v.iter().all(|x| x.is_finite() && (0. ..=1.).contains(x))
}
fn rotation(v: [f32; 3]) -> Quat {
    Quat::from_euler(EulerRot::XYZ, v[0], v[1], v[2])
}
impl Body {
    /// Primitive surface bounds in the saved rest pose (without proxy padding).
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut lower = Vec3::splat(f32::INFINITY);
        let mut upper = Vec3::splat(f32::NEG_INFINITY);
        for bone in &self.bones {
            let (start, end) = self.bone_segment(bone.id)?;
            let end = Vec3::from_array(end);
            let start = match bone.shape {
                BodyShape::Sphere => end,
                BodyShape::Capsule => Vec3::from_array(start),
            };
            lower = lower.min(start.min(end) - Vec3::splat(bone.radius));
            upper = upper.max(start.max(end) + Vec3::splat(bone.radius));
        }
        (!self.bones.is_empty()).then_some((lower, upper))
    }

    pub fn bone_segment(&self, id: u32) -> Option<([f32; 3], [f32; 3])> {
        let mut chain = vec![];
        let mut current = Some(id);
        let mut seen = HashSet::new();
        while let Some(id) = current {
            if !seen.insert(id) {
                return None;
            }
            let bone = self.bones.iter().find(|b| b.id == id)?;
            chain.push(bone);
            current = bone.parent;
        }
        let mut end = Vec3::ZERO;
        let mut start = Vec3::ZERO;
        for bone in chain.into_iter().rev() {
            start = end + Vec3::from_array(bone.offset);
            end = start + Vec3::from_array(bone.tip);
        }
        Some((start.to_array(), end.to_array()))
    }
    pub fn remove_bone(&mut self, id: u32) -> Vec<u32> {
        if !self.bones.iter().any(|b| b.id == id) {
            return vec![];
        }
        let mut removed = vec![id];
        loop {
            let children: Vec<_> = self
                .bones
                .iter()
                .filter(|b| {
                    !removed.contains(&b.id) && b.parent.is_some_and(|p| removed.contains(&p))
                })
                .map(|b| b.id)
                .collect();
            if children.is_empty() {
                break;
            }
            removed.extend(children);
        }
        self.bones.retain(|b| !removed.contains(&b.id));
        self.mounts.retain(|m| !removed.contains(&m.bone));
        removed
    }
}
impl VoxelBlock {
    pub fn contains(&self, p: [i32; 3]) -> bool {
        (0..3).all(|i| {
            p[i] >= self.min[i] && i64::from(p[i]) < i64::from(self.min[i]) + i64::from(self.size)
        })
    }
    fn validate(&self) -> Result<(), String> {
        if ![1, 2, 4, 8, 16, 32].contains(&self.size)
            || !color_valid(self.color)
            || self
                .min
                .iter()
                .any(|p| *p < 0 || i64::from(*p) + i64::from(self.size) > i64::from(GRID_SIZE))
        {
            return Err("Invalid voxel block: expected a power-of-two size inside the 32³ grid and RGB in 0..1".into());
        }
        Ok(())
    }
}
impl Library {
    fn ids(&self) -> Vec<u32> {
        let mut ids = vec![];
        for b in &self.bodies {
            ids.push(b.id);
            ids.extend(b.bones.iter().map(|b| b.id));
            ids.extend(b.mounts.iter().map(|m| m.id));
        }
        for p in &self.props {
            ids.push(p.id);
            ids.extend(p.anchors.iter().map(|a| a.id));
        }
        for c in &self.characters {
            ids.push(c.id);
            ids.extend(c.attachments.iter().map(|a| a.id));
        }
        ids
    }
    pub fn next_id(&self) -> u32 {
        self.ids().into_iter().max().unwrap_or(0).saturating_add(1)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!("Unsupported library version {}", self.version));
        }
        let mut ids = HashSet::new();
        for id in self.ids() {
            if id == 0 || id == u32::MAX || !ids.insert(id) {
                return Err(format!("Invalid or duplicate ID {id}"));
            }
        }
        for b in &self.bodies {
            if b.bones.len() > MAX_BONES {
                return Err(format!("Body {} exceeds the {MAX_BONES}-bone limit", b.id));
            }
            for bone in &b.bones {
                if !finite(bone.offset)
                    || !finite(bone.tip)
                    || !bone.radius.is_finite()
                    || !(0.01..=10.).contains(&bone.radius)
                    || !color_valid(bone.color)
                {
                    return Err(format!("Invalid parameters on bone {}", bone.id));
                }
                if b.bone_segment(bone.id).is_none() {
                    return Err(format!("Cycle or missing parent on bone {}", bone.id));
                }
            }
            for m in &b.mounts {
                if !b.bones.iter().any(|bone| bone.id == m.bone)
                    || !finite(m.offset)
                    || !finite(m.rotation)
                {
                    return Err(format!("Invalid mount {}", m.id));
                }
            }
        }
        for p in &self.props {
            // A compact occupancy bitset bounds validation cost even for many
            // fine blocks. One u32 represents a complete row of the 32³ grid.
            let mut rows = [0_u32; 32 * 32];
            for b in &p.blocks {
                b.validate()?;
                let mask = if b.size == 32 {
                    u32::MAX
                } else {
                    ((1_u32 << b.size) - 1) << b.min[0]
                };
                for z in b.min[2]..b.min[2] + b.size as i32 {
                    for y in b.min[1]..b.min[1] + b.size as i32 {
                        let row = &mut rows[(z * 32 + y) as usize];
                        if *row & mask != 0 {
                            return Err(format!("Overlapping blocks in prop {}", p.id));
                        }
                        *row |= mask;
                    }
                }
            }
            for a in &p.anchors {
                if !finite(a.position) || !finite(a.rotation) {
                    return Err(format!("Invalid anchor {}", a.id));
                }
            }
        }
        for c in &self.characters {
            let b = self
                .bodies
                .iter()
                .find(|b| b.id == c.body)
                .ok_or_else(|| format!("Missing body for character {}", c.id))?;
            crate::animation::validate_clips(&c.animations, b)?;
            for a in &c.attachments {
                let p = self
                    .props
                    .iter()
                    .find(|p| p.id == a.prop)
                    .ok_or_else(|| format!("Missing prop for attachment {}", a.id))?;
                if !b.mounts.iter().any(|m| m.id == a.mount)
                    || !p.anchors.iter().any(|anchor| anchor.id == a.anchor)
                    || !finite(a.offset)
                    || !finite(a.rotation)
                    || !a.scale.is_finite()
                    || !(0.01..=20.).contains(&a.scale)
                {
                    return Err(format!("Invalid attachment {}", a.id));
                }
            }
        }
        Ok(())
    }
}
/// Compose mount * edit * inverse(anchor), with scale about the anchor.
pub fn attachment_transform(
    body: &Body,
    mount: &Mount,
    prop: &Prop,
    anchor: &Anchor,
    attachment: &Attachment,
) -> Option<Transform> {
    if mount.id != attachment.mount
        || prop.id != attachment.prop
        || anchor.id != attachment.anchor
        || !prop.anchors.iter().any(|a| a.id == anchor.id)
    {
        return None;
    }
    let (_, end) = body.bone_segment(mount.bone)?;
    let mount_rotation = rotation(mount.rotation);
    let target = Vec3::from_array(end)
        + Vec3::from_array(mount.offset)
        + mount_rotation * Vec3::from_array(attachment.offset);
    let rotation =
        mount_rotation * rotation(attachment.rotation) * rotation(anchor.rotation).inverse();
    Some(Transform {
        translation: target - rotation * (Vec3::from_array(anchor.position) * attachment.scale),
        rotation,
        scale: Vec3::splat(attachment.scale),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bone(id: u32, parent: Option<u32>) -> Bone {
        Bone {
            id,
            name: format!("Bone {id}"),
            parent,
            offset: [0.; 3],
            tip: [0., 1., 0.],
            radius: 0.2,
            shape: BodyShape::Capsule,
            color: [0.2, 0.6, 0.8],
        }
    }
    fn body() -> Body {
        Body {
            id: 1,
            name: "Body".into(),
            bones: vec![bone(2, None), bone(3, Some(2)), bone(4, Some(3))],
            mounts: vec![Mount {
                id: 5,
                name: "Head".into(),
                bone: 4,
                offset: [0., 0.2, 0.],
                rotation: [0.; 3],
            }],
        }
    }
    #[test]
    fn hierarchy_and_cascade() {
        let mut b = body();
        b.bones[1].offset = [0.5, 0., 0.];
        assert_eq!(b.bone_segment(4), Some(([0.5, 2., 0.], [0.5, 3., 0.])));
        assert_eq!(b.remove_bone(3), vec![3, 4]);
        assert_eq!(b.bones.len(), 1);
        assert!(b.mounts.is_empty());
    }
    #[test]
    fn rejects_cycles_duplicates_references_and_nonfinite() {
        let mut l = Library {
            bodies: vec![body()],
            ..Default::default()
        };
        assert!(l.validate().is_ok());
        l.bodies[0].bones[0].parent = Some(4);
        assert!(l.validate().is_err());
        l.bodies[0].bones[0].parent = None;
        l.bodies[0].bones[0].radius = f32::NAN;
        assert!(l.validate().is_err());
        l.bodies[0].bones[0].radius = 0.2;
        l.bodies[0].mounts[0].bone = 99;
        assert!(l.validate().is_err());
        l.bodies[0].mounts[0].bone = 4;
        l.bodies[0].bones[0].id = 1;
        assert!(l.validate().is_err());
    }
    #[test]
    fn body_limit_and_attachment_references_are_validated() {
        let mut library = Library {
            bodies: vec![body()],
            props: vec![Prop {
                id: 6,
                name: "Hat".into(),
                blocks: vec![],
                anchors: vec![Anchor {
                    id: 7,
                    name: "Base".into(),
                    position: [0.; 3],
                    rotation: [0.; 3],
                }],
            }],
            characters: vec![Character {
                id: 8,
                name: "Player".into(),
                body: 1,
                animations: vec![],
                attachments: vec![Attachment {
                    id: 9,
                    prop: 6,
                    anchor: 7,
                    mount: 5,
                    offset: [0.; 3],
                    rotation: [0.; 3],
                    scale: 1.,
                }],
            }],
            ..Default::default()
        };
        assert!(library.validate().is_ok());
        assert_eq!(library.next_id(), 10);
        let restored: Library =
            serde_json::from_str(&serde_json::to_string(&library).unwrap()).unwrap();
        assert_eq!(restored, library);
        library.characters[0].attachments[0].anchor = 99;
        assert!(library.validate().is_err());
        library.characters[0].attachments[0].anchor = 7;
        library.characters[0].attachments[0].scale = 0.;
        assert!(library.validate().is_err());
        library.characters[0].attachments[0].scale = 1.;
        for id in 10..43 {
            library.bodies[0].bones.push(bone(id, None));
        }
        assert!(library.validate().is_err());
    }
    #[test]
    fn rejects_overlapping_and_out_of_bounds_blocks() {
        let block = VoxelBlock {
            min: [0; 3],
            size: 16,
            color: [1.; 3],
        };
        let mut library = Library {
            props: vec![Prop {
                id: 1,
                name: "Hat".into(),
                blocks: vec![block.clone()],
                anchors: vec![],
            }],
            ..Default::default()
        };
        assert!(library.validate().is_ok());
        library.props[0].blocks.push(block);
        assert!(library.validate().is_err());
        library.props[0].blocks.pop();
        library.props[0].blocks[0].min = [31; 3];
        assert!(library.validate().is_err());
    }
    #[test]
    fn rotated_scaled_anchor_aligns_with_mount() {
        let mut b = body();
        b.mounts[0].rotation = [0., 0., std::f32::consts::FRAC_PI_2];
        let anchor = Anchor {
            id: 7,
            name: "Base".into(),
            position: [0.3, 0.1, -0.2],
            rotation: [0.4, 0.2, 0.1],
        };
        let p = Prop {
            id: 6,
            name: "Hat".into(),
            blocks: vec![],
            anchors: vec![anchor.clone()],
        };
        let a = Attachment {
            id: 8,
            prop: 6,
            anchor: 7,
            mount: 5,
            offset: [0.5, 0., 0.],
            rotation: [0.1, 0.2, 0.3],
            scale: 2.,
        };
        let t = attachment_transform(&b, &b.mounts[0], &p, &anchor, &a).unwrap();
        let point = t.transform_point(Vec3::from_array(anchor.position));
        assert!(point.distance(Vec3::new(0., 3.7, 0.)) < 0.0001);
        let expected = rotation(b.mounts[0].rotation) * rotation(a.rotation);
        assert!((t.rotation * rotation(anchor.rotation)).angle_between(expected) < 0.001);
    }
}
