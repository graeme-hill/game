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
    /// Ground-relative authored gait: distance travelled per complete cycle.
    /// Zero denotes a stationary pose. None retains legacy surface grounding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stride_distance: Option<f32>,
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
            || clip
                .stride_distance
                .is_some_and(|d| !d.is_finite() || !(0. ..=100.).contains(&d))
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
            stride_distance: None,
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

/// Editable starter clips, baked from foot contacts and a two-bone sagittal solve.
/// The runtime still samples ordinary keys; there is no special-case bone solver
/// in playback. Model forward is -Z: knees point forward and heels fold back.
pub fn locomotion_clips(body: &Body) -> Vec<AnimationClip> {
    ["standing", "idle", "walking", "running"]
        .into_iter()
        .map(|name| bake_gait(body, name))
        .collect()
}

fn bone_named(body: &Body, name: &str) -> Option<usize> {
    body.bones
        .iter()
        .position(|b| b.name.eq_ignore_ascii_case(name))
}

fn leg_pair(body: &Body, side: &str) -> Option<(usize, usize)> {
    let upper = bone_named(body, &format!("{side} upper leg"))?;
    let lower = bone_named(body, &format!("{side} lower leg"))?;
    (body.bones[lower].parent == Some(body.bones[upper].id)).then_some((upper, lower))
}

fn sagittal_length(tip: [f32; 3]) -> f32 {
    tip[1].hypot(tip[2])
}

/// Smooth interpolation between authored contact, down, passing and up heights.
fn gait_height(phase: f32, running: bool) -> f32 {
    let keys: &[(f32, f32)] = if running {
        &[
            (0., 0.88),
            (0.10, 0.72),
            (0.25, 0.86),
            (0.35, 0.90),
            (0.43, 1.01),
            (0.5, 0.88),
        ]
    } else {
        &[
            (0., 0.86),
            (0.10, 0.82),
            (0.27, 0.97),
            (0.40, 0.93),
            (0.5, 0.86),
        ]
    };
    let phase = phase.rem_euclid(0.5);
    let pair = keys.windows(2).find(|k| phase <= k[1].0).unwrap();
    let t = (phase - pair[0].0) / (pair[1].0 - pair[0].0);
    let t = t * t * (3. - 2. * t);
    pair[0].1 + (pair[1].1 - pair[0].1) * t
}

/// A planted foot moves backward at travel speed in model space. The recovery
/// arc matches that velocity at both ends, lifting the heel behind the knee.
fn foot_path(phase: f32, length: f32, stride: f32, running: bool) -> (f32, f32) {
    let stance = if running { 0.35 } else { 0.6 };
    let front = stride * stance * 0.5;
    if phase <= stance {
        return (front - stride * phase, 0.);
    }
    let t = (phase - stance) / (1. - stance);
    let tangent = -stride * (1. - stance);
    let t2 = t * t;
    let t3 = t2 * t;
    let forward = (2. * t3 - 3. * t2 + 1.) * -front
        + (t3 - 2. * t2 + t) * tangent
        + (-2. * t3 + 3. * t2) * front
        + (t3 - t2) * tangent;
    let lift =
        (std::f32::consts::PI * t).sin().powi(2) * length * if running { 0.55 } else { 0.22 };
    (forward, lift)
}

fn bake_gait(body: &Body, name: &str) -> AnimationClip {
    let moving = matches!(name, "walking" | "running");
    let running = name == "running";
    let duration = match name {
        "walking" => 1.,
        "running" => 0.65,
        "idle" => 4.,
        _ => 2.5,
    };
    let torso = bone_named(body, "Torso");
    let pairs = [leg_pair(body, "Left"), leg_pair(body, "Right")];
    let grounded = torso.is_some() && pairs.iter().all(Option::is_some);
    let rest = forward_kinematics(body, &[]);
    let floor = body.bounds().map_or(0., |b| b.0.y);
    let length = pairs[0].map_or(1., |(u, l)| {
        sagittal_length(body.bones[u].tip) + sagittal_length(body.bones[l].tip)
    });
    let stride = if !moving {
        0.
    } else {
        length * if running { 2.4 } else { 1.6 }
    };
    let mut tracks: Vec<_> = body
        .bones
        .iter()
        .map(|bone| BoneTrack {
            bone: bone.id,
            keys: vec![],
        })
        .collect();
    for step in 0..=64 {
        let phase = step as f32 / 64.;
        let wave = (phase * std::f32::consts::TAU).sin();
        let mut locals = vec![LocalPose::default(); body.bones.len()];
        let lean = if moving {
            if running { -0.20 } else { -0.055 }
        } else {
            0.
        };
        if let Some(torso) = torso {
            locals[torso].rotation = Quat::from_rotation_x(lean);
            if let Some((_, lower)) = pairs[0].filter(|_| grounded) {
                let height = length
                    * if moving {
                        gait_height(phase, running)
                    } else {
                        0.985 + wave * 0.003
                    };
                locals[torso].translation.y =
                    floor + body.bones[lower].radius + height - rest[torso].start.y;
            }
        }
        if let Some(head) = bone_named(body, "Head") {
            locals[head].rotation = Quat::from_rotation_x(-lean)
                * Quat::from_rotation_y(if name == "idle" { wave * 0.09 } else { 0. });
        }
        let root_frames = forward_kinematics(body, &locals);
        for (side_index, side) in ["Left", "Right"].iter().enumerate() {
            let cycle = (phase + side_index as f32 * 0.5).rem_euclid(1.);
            if let Some((upper, lower)) = pairs[side_index].filter(|_| grounded) {
                let (forward, lift) = if moving {
                    foot_path(cycle, length, stride, running)
                } else {
                    (0., 0.)
                };
                let hip = root_frames[upper].start;
                let down = hip.y - (floor + body.bones[lower].radius + lift);
                let forward = forward + hip.z - rest[upper].start.z;
                let a = sagittal_length(body.bones[upper].tip).max(0.001);
                let b = sagittal_length(body.bones[lower].tip).max(0.001);
                let reach2 = (down * down + forward * forward)
                    .clamp((a - b).powi(2) + 0.000001, (a + b).powi(2) - 0.000001);
                let bend = ((reach2 - a * a - b * b) / (2. * a * b))
                    .clamp(-1., 1.)
                    .acos();
                let thigh = forward.atan2(down) + (b * bend.sin()).atan2(a + b * bend.cos());
                let upper_rest = (-body.bones[upper].tip[2]).atan2(-body.bones[upper].tip[1]);
                let lower_rest = (-body.bones[lower].tip[2]).atan2(-body.bones[lower].tip[1]);
                let parent_rotation = body.bones[upper]
                    .parent
                    .and_then(|id| body.bones.iter().position(|b| b.id == id))
                    .map_or(Quat::IDENTITY, |i| root_frames[i].rotation);
                locals[upper].rotation =
                    parent_rotation.inverse() * Quat::from_rotation_x(thigh - upper_rest);
                locals[lower].rotation = Quat::from_rotation_x(-bend + upper_rest - lower_rest);
            }
            let swing = (cycle * std::f32::consts::TAU).cos();
            if let Some(upper) = bone_named(body, &format!("{side} upper arm"))
                .or_else(|| bone_named(body, &format!("{side} arm")))
            {
                // Arms oppose the same-side foot; shoulders stay relaxed.
                let angle = if moving {
                    -swing * if running { 0.72 } else { 0.38 } - if running { 0.12 } else { 0. }
                } else {
                    if name == "idle" { wave * 0.025 } else { 0. }
                };
                locals[upper].rotation = Quat::from_rotation_x(angle - lean);
            }
            if let Some(lower) = bone_named(body, &format!("{side} lower arm")) {
                locals[lower].rotation = Quat::from_rotation_x(if moving {
                    if running {
                        1.12 - 0.15 * swing
                    } else {
                        0.28 - 0.08 * swing
                    }
                } else {
                    0.16
                });
            }
        }
        for (track, local) in tracks.iter_mut().zip(locals) {
            let (x, y, z) = local.rotation.to_euler(EulerRot::XYZ);
            track.keys.push(AnimationKey {
                time: phase * duration,
                rotation: [x, y, z],
                translation: local.translation.to_array(),
            });
        }
    }
    tracks.retain(|t| {
        t.keys
            .iter()
            .any(|k| k.rotation != [0.; 3] || k.translation != [0.; 3])
    });
    AnimationClip {
        name: name.into(),
        duration,
        looping: true,
        tracks,
        stride_distance: grounded.then_some(stride),
    }
}

#[cfg(test)]
mod document_tests {
    use super::*;
    #[test]
    fn old_characters_load_and_invalid_animation_documents_are_rejected() {
        let legacy: AnimationClip =
            serde_json::from_str(r#"{"name":"walking","duration":1.0,"looping":true,"tracks":[]}"#)
                .unwrap();
        assert_eq!(legacy.stride_distance, None);
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
        broken[0].stride_distance = Some(-1.);
        assert!(validate_clips(&broken, body).is_err());
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

#[cfg(test)]
mod gait_tests {
    use super::*;

    #[test]
    fn baked_gaits_keep_joints_anatomical_and_stance_feet_planted() {
        let library = crate::storage::load_workspace(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("test_workspace"),
        )
        .unwrap();
        let body = &library.bodies[0];
        let floor = body.bounds().unwrap().0.y;
        let generated = locomotion_clips(body);
        // Check the actual startup documents as well as newly generated clips.
        for clip in library.characters[0].animations.iter().chain(&generated) {
            let running = clip.name == "running";
            let moving = matches!(clip.name.as_str(), "walking" | "running");
            let stride = clip.stride_distance.unwrap();
            let stance = if running { 0.35 } else { 0.6 };
            let at = |phase| {
                forward_kinematics(
                    body,
                    &blend_clips(body, &[(clip, phase * clip.duration, 1.)]),
                )
            };
            let start = at(0.);
            for sample in 0..256 {
                let phase = sample as f32 / 256.;
                let frames = at(phase);
                for (side_index, side) in ["Left", "Right"].iter().enumerate() {
                    let (upper, lower) = leg_pair(body, side).unwrap();
                    let thigh = (frames[upper].end - frames[upper].start).normalize();
                    let shin = (frames[lower].end - frames[lower].start).normalize();
                    assert!(
                        thigh.cross(shin).x <= 0.0001,
                        "{} {phase}: knee bent backwards",
                        clip.name
                    );
                    assert!(frames[lower].start.distance(frames[upper].end) < 0.00001);
                    let cycle = (phase + side_index as f32 * 0.5).rem_euclid(1.);
                    let foot_height = frames[lower].end.y - body.bones[lower].radius - floor;
                    assert!(
                        foot_height > -0.007,
                        "{} {phase}: foot penetrates ground: {foot_height}",
                        clip.name
                    );
                    if !moving || cycle <= stance {
                        assert!(
                            foot_height.abs() < 0.007,
                            "{} {phase}: stance foot floats: {foot_height}",
                            clip.name
                        );
                    }
                    if moving && side_index == 0 && phase <= stance {
                        let world_z = frames[lower].end.z - stride * phase;
                        assert!(
                            (world_z - start[lower].end.z).abs() < 0.007,
                            "{} {phase}: planted foot slides",
                            clip.name
                        );
                    }
                    let upper_arm = bone_named(body, &format!("{side} upper arm")).unwrap();
                    let lower_arm = bone_named(body, &format!("{side} lower arm")).unwrap();
                    let arm = frames[upper_arm].end - frames[upper_arm].start;
                    let forearm = frames[lower_arm].end - frames[lower_arm].start;
                    assert!(arm.cross(forearm).x >= 0., "elbow bent backwards");
                }
                let torso = bone_named(body, "Torso").unwrap();
                let head = bone_named(body, "Head").unwrap();
                assert!(
                    frames[torso].end.z <= frames[torso].start.z + 0.00001,
                    "backward lean"
                );
                assert!(
                    (frames[head].rotation * Vec3::Y).distance(Vec3::Y) < 0.00001,
                    "head follows torso pitch"
                );
            }
            let end = at(1.);
            assert!(
                start
                    .iter()
                    .zip(end)
                    .all(|(a, b)| a.end.distance(b.end) < 0.00001)
            );
            if running {
                let flight = at(0.43);
                for side in ["Left", "Right"] {
                    let (_, lower) = leg_pair(body, side).unwrap();
                    assert!(
                        flight[lower].end.y - body.bones[lower].radius - floor > 0.01,
                        "run has no flight phase"
                    );
                }
            }
        }
    }
}
