//! Serializable sparse keyframes and pure sampling/blending/forward kinematics.
use crate::model::{Body, BodyShape};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationClip {
    pub name: String,
    pub duration: f32,
    pub looping: bool,
    pub tracks: Vec<BoneTrack>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BoneTrack {
    pub bone: u32,
    pub keys: Vec<AnimationKey>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnimationKey {
    pub time: f32,
    /// Euler XYZ radians, relative to the bone's rest pose.
    pub rotation: [f32; 3],
    pub translation: [f32; 3],
}
#[derive(Clone, Copy, Debug, Default)]
pub struct LocalPose {
    pub rotation: Quat,
    pub translation: Vec3,
}
#[derive(Clone, Copy, Debug)]
pub struct BoneFrame {
    pub start: Vec3,
    pub end: Vec3,
    pub rotation: Quat,
}

pub fn euler(values: [f32; 3]) -> Quat {
    Quat::from_euler(EulerRot::XYZ, values[0], values[1], values[2])
}
impl From<&AnimationKey> for LocalPose {
    fn from(key: &AnimationKey) -> Self {
        Self {
            rotation: euler(key.rotation),
            translation: Vec3::from_array(key.translation),
        }
    }
}
impl LocalPose {
    pub fn blend(self, other: Self, amount: f32) -> Self {
        Self {
            rotation: self.rotation.slerp(other.rotation, amount),
            translation: self.translation.lerp(other.translation, amount),
        }
    }
}

pub fn validate_clips(clips: &[AnimationClip], body: &Body) -> Result<(), String> {
    let mut names = HashSet::new();
    for clip in clips {
        if clip.name.trim().is_empty()
            || !names.insert(clip.name.to_lowercase())
            || !clip.duration.is_finite()
            || !(0.05..=3600.).contains(&clip.duration)
        {
            return Err(format!("Invalid or duplicate animation: {}", clip.name));
        }
        let mut bones = HashSet::new();
        for track in &clip.tracks {
            if !body.bones.iter().any(|b| b.id == track.bone) || !bones.insert(track.bone) {
                return Err(format!(
                    "Missing or duplicate bone {} in animation {}",
                    track.bone, clip.name
                ));
            }
            let mut previous = -1.;
            for key in &track.keys {
                if !key.time.is_finite()
                    || key.time < 0.
                    || key.time > clip.duration
                    || key.time <= previous
                    || key
                        .rotation
                        .iter()
                        .chain(&key.translation)
                        .any(|v| !v.is_finite() || v.abs() > 1000.)
                {
                    return Err(format!("Invalid keyframe in animation {}", clip.name));
                }
                previous = key.time;
            }
        }
    }
    Ok(())
}

pub fn sample_track(track: &BoneTrack, time: f32, duration: f32, looping: bool) -> LocalPose {
    let keys = &track.keys;
    let Some(first) = keys.first() else {
        return LocalPose::default();
    };
    if keys.len() == 1 {
        return first.into();
    }
    let time = if looping {
        time.rem_euclid(duration)
    } else {
        time.clamp(0., duration)
    };
    let last = keys.last().unwrap();
    let (a, b, t) = if time < first.time || time > last.time {
        if !looping {
            return if time < first.time {
                first.into()
            } else {
                last.into()
            };
        }
        let span = duration - last.time + first.time;
        let elapsed = if time < first.time {
            time + duration - last.time
        } else {
            time - last.time
        };
        (last, first, elapsed / span.max(f32::EPSILON))
    } else {
        let right = keys
            .partition_point(|key| key.time < time)
            .clamp(1, keys.len() - 1);
        let a = &keys[right - 1];
        let b = &keys[right];
        (a, b, (time - a.time) / (b.time - a.time))
    };
    LocalPose::from(a).blend(b.into(), t.clamp(0., 1.))
}

/// Any number of weighted clips. Missing tracks contribute the rest pose.
/// Times are per clip; callers can synchronize walk/run with normalized phase.
pub fn blend_clips(body: &Body, clips: &[(&AnimationClip, f32, f32)]) -> Vec<LocalPose> {
    body.bones
        .iter()
        .map(|bone| {
            let mut pose = LocalPose::default();
            let mut total = 0.;
            for &(clip, time, weight) in clips {
                if !weight.is_finite() || weight <= 0. {
                    continue;
                }
                let next = clip
                    .tracks
                    .iter()
                    .find(|track| track.bone == bone.id)
                    .map(|track| sample_track(track, time, clip.duration, clip.looping))
                    .unwrap_or_default();
                total += weight;
                pose = pose.blend(next, weight / total);
            }
            pose
        })
        .collect()
}

/// Parent rotation carries child offsets and joints; bone lengths stay constant.
pub fn forward_kinematics(body: &Body, locals: &[LocalPose]) -> Vec<BoneFrame> {
    fn solve(
        index: usize,
        body: &Body,
        locals: &[LocalPose],
        cache: &mut [Option<BoneFrame>],
    ) -> BoneFrame {
        if let Some(frame) = cache[index] {
            return frame;
        }
        let bone = &body.bones[index];
        let parent = bone
            .parent
            .and_then(|id| body.bones.iter().position(|b| b.id == id))
            .map(|parent| solve(parent, body, locals, cache));
        let rotation = parent.map_or(Quat::IDENTITY, |frame| frame.rotation);
        let origin = parent.map_or(Vec3::ZERO, |frame| frame.end);
        let local = locals.get(index).copied().unwrap_or_default();
        let start = origin + rotation * (Vec3::from_array(bone.offset) + local.translation);
        let rotation = rotation * local.rotation;
        let frame = BoneFrame {
            start,
            end: start + rotation * Vec3::from_array(bone.tip),
            rotation,
        };
        cache[index] = Some(frame);
        frame
    }
    let mut cache = vec![None; body.bones.len()];
    (0..body.bones.len())
        .map(|index| solve(index, body, locals, &mut cache))
        .collect()
}

pub fn pose_bounds(body: &Body, frames: &[BoneFrame]) -> Option<(Vec3, Vec3)> {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for (bone, frame) in body.bones.iter().zip(frames) {
        let start = if bone.shape == BodyShape::Sphere {
            frame.end
        } else {
            frame.start
        };
        min = min.min(start.min(frame.end) - Vec3::splat(bone.radius));
        max = max.max(start.max(frame.end) + Vec3::splat(bone.radius));
    }
    min.is_finite().then_some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn key(time: f32, degrees: f32) -> AnimationKey {
        AnimationKey {
            time,
            rotation: [0., 0., degrees.to_radians()],
            translation: [time, 0., 0.],
        }
    }
    #[test]
    fn interpolation_wrap_clamp_and_shortest_rotation() {
        let track = BoneTrack {
            bone: 1,
            keys: vec![key(0.25, 170.), key(0.75, -170.)],
        };
        let middle = sample_track(&track, 0.5, 1., true);
        assert!((middle.rotation * Vec3::X + Vec3::X).length() < 0.0001);
        assert!((middle.translation.x - 0.5).abs() < 0.0001);
        assert!((sample_track(&track, 1., 1., true).translation.x - 0.5).abs() < 0.0001);
        assert_eq!(sample_track(&track, 5., 1., false).translation.x, 0.75);
    }
    #[test]
    fn hierarchy_and_blend_preserve_limb_lengths_and_joint_connections() {
        let library = crate::storage::load_workspace(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_workspace"),
        )
        .unwrap();
        let body = &library.bodies[0];
        let clip = AnimationClip {
            name: "test".into(),
            duration: 1.,
            looping: true,
            tracks: vec![BoneTrack {
                bone: body.bones[0].id,
                keys: vec![key(0., 90.)],
            }],
        };
        let rest = AnimationClip {
            tracks: vec![],
            ..clip.clone()
        };
        let locals = blend_clips(body, &[(&rest, 0., 0.5), (&clip, 0., 0.5)]);
        assert!(
            (locals[0].rotation * Vec3::Y - Vec3::new(-0.5_f32.sqrt(), 0.5_f32.sqrt(), 0.))
                .length()
                < 0.0001
        );
        let frames = forward_kinematics(body, &locals);
        for (bone, frame) in body.bones.iter().zip(&frames) {
            assert!(
                ((frame.end - frame.start).length() - Vec3::from_array(bone.tip).length()).abs()
                    < 0.0001
            );
        }
        assert!((frames[3].start - frames[2].end).length() < 0.0001);
        assert!(validate_clips(&[clip.clone(), clip], body).is_err());
    }
}

/// Editable starter clips for the workshop's named humanoid bones.
/// Stored as ordinary keys; playback has no procedural/special-case limb motion.
pub fn locomotion_clips(body: &Body) -> Vec<AnimationClip> {
    ["standing", "idle", "walking", "running"]
        .into_iter()
        .map(|name| {
            let moving = name == "walking" || name == "running";
            let running = name == "running";
            let duration = match name {
                "walking" => 1.,
                "running" => 0.65,
                "idle" => 4.,
                _ => 2.5,
            };
            let tracks = body
                .bones
                .iter()
                .filter_map(|bone| {
                    let label = bone.name.to_lowercase();
                    let side = if label.contains("left") { 1. } else { -1. };
                    let keys = (0..=16)
                        .map(|step| {
                            let phase = step as f32 / 16.;
                            let wave = (phase * std::f32::consts::TAU).sin();
                            let stride = wave * side;
                            let mut rotation = [0.; 3];
                            let mut translation = [0.; 3];
                            if moving {
                                if label.contains("upper leg") {
                                    rotation[0] = stride * if running { 0.95 } else { 0.5 };
                                } else if label.contains("lower leg") {
                                    rotation[0] = -0.08
                                        - (-stride).max(0.) * if running { 1.35 } else { 0.65 };
                                } else if label.contains("upper arm")
                                    || (label.ends_with(" arm") && !label.contains("lower"))
                                {
                                    rotation[0] = -stride * if running { 0.85 } else { 0.4 };
                                } else if label.contains("lower arm") {
                                    rotation[0] = if running { 1.15 } else { 0.25 };
                                } else if label == "torso" {
                                    rotation[0] = if running { -0.12 } else { -0.025 };
                                    rotation[1] = wave * if running { 0.07 } else { 0.025 };
                                    translation[1] =
                                        (wave * wave) * if running { 0.08 } else { 0.035 };
                                } else if label == "head" {
                                    rotation[0] = if running { 0.12 } else { 0.025 };
                                }
                            } else if label == "torso" {
                                translation[1] = wave * if name == "idle" { 0.018 } else { 0.004 };
                                rotation[2] = wave * if name == "idle" { 0.018 } else { 0.003 };
                            } else if label == "head" && name == "idle" {
                                rotation[1] = wave * 0.12;
                            } else if label.contains("arm") && name == "idle" {
                                rotation[0] = wave * side * 0.035;
                            }
                            AnimationKey {
                                time: phase * duration,
                                rotation,
                                translation,
                            }
                        })
                        .collect::<Vec<_>>();
                    keys.iter()
                        .any(|key| key.rotation != [0.; 3] || key.translation != [0.; 3])
                        .then_some(BoneTrack {
                            bone: bone.id,
                            keys,
                        })
                })
                .collect();
            AnimationClip {
                name: name.into(),
                duration,
                looping: true,
                tracks,
            }
        })
        .collect()
}

#[cfg(test)]
mod document_tests {
    use super::*;
    #[test]
    fn old_characters_load_and_invalid_animation_documents_are_rejected() {
        let character: crate::model::Character =
            serde_json::from_str(r#"{"id":12,"name":"Old","body":1,"attachments":[]}"#).unwrap();
        assert!(character.animations.is_empty());
        let lib = crate::storage::load_workspace(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_workspace"),
        )
        .unwrap();
        let body = &lib.bodies[0];
        let clips = locomotion_clips(body);
        validate_clips(&clips, body).unwrap();
        let encoded = serde_json::to_string(&clips).unwrap();
        assert_eq!(
            clips,
            serde_json::from_str::<Vec<AnimationClip>>(&encoded).unwrap()
        );
        let mut broken = clips.clone();
        broken[1].name = "STANDING".into();
        assert!(validate_clips(&broken, body).is_err());
        let mut broken = clips.clone();
        broken[0].tracks[0].bone = u32::MAX;
        assert!(validate_clips(&broken, body).is_err());
        let mut broken = clips.clone();
        broken[0].tracks[0].keys[1].time = 0.;
        assert!(validate_clips(&broken, body).is_err());
        let mut broken = clips;
        broken[0].tracks[0].keys[1].rotation[0] = f32::NAN;
        assert!(validate_clips(&broken, body).is_err());
    }
}
