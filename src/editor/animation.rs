//! Character animation editing uses normal document undo/save and the shared
//! sampling code used by play mode.
use super::*;
use game::{
    animation::{
        AnimationClip, AnimationKey, BoneTrack, LocalPose, blend_clips, locomotion_clips,
        sample_track,
    },
    character_render::VisualPose,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimOp {
    New,
    Duplicate,
    Delete,
    Presets,
    Play,
    Loop,
    Key,
    DeleteKey,
    Step(f32),
    PrevKey,
    NextKey,
    Start,
    End,
    BlendWith(Option<usize>),
    Close,
}

impl Editor {
    pub fn animation_clip(&self) -> Option<&AnimationClip> {
        self.active_character()?.animations.get(self.anim.clip?)
    }
    fn selected_anim_bone(&self) -> Option<u32> {
        match self.sel() {
            Sel::Bone(id) if self.active_kind() == Some(DocKind::Character) => Some(id),
            _ => None,
        }
    }
    /// The selected bone's pose in the current clip at the timeline cursor.
    pub fn animation_values(&self) -> Option<LocalPose> {
        let bone = self.selected_anim_bone()?;
        let clip = self.animation_clip()?;
        Some(
            clip.tracks
                .iter()
                .find(|t| t.bone == bone)
                .map(|track| sample_track(track, self.anim.time, clip.duration, false))
                .unwrap_or_default(),
        )
    }
    /// Key times of the selected bone's track (or all tracks with no bone).
    pub fn key_times(&self) -> Vec<f32> {
        let Some(clip) = self.animation_clip() else {
            return vec![];
        };
        let bone = self.selected_anim_bone();
        let mut times: Vec<f32> = clip
            .tracks
            .iter()
            .filter(|t| bone.is_none_or(|b| t.bone == b))
            .flat_map(|t| t.keys.iter().map(|k| k.time))
            .collect();
        times.sort_by(f32::total_cmp);
        times.dedup_by(|a, b| (*a - *b).abs() < 0.0005);
        times
    }
}

fn clip_mut(e: &mut Editor) -> Result<&mut AnimationClip, String> {
    let index = e.anim.clip.ok_or("Select an animation")?;
    e.active_character_mut()
        .and_then(|c| c.animations.get_mut(index))
        .ok_or_else(|| "Select an animation".into())
}

pub(super) fn perform(e: &mut Editor, op: AnimOp) -> Result<(), String> {
    match op {
        AnimOp::New | AnimOp::Duplicate => {
            let source = e.animation_clip().cloned();
            let c = e.active_character_mut().ok_or("Open a character")?;
            let mut clip = if op == AnimOp::Duplicate {
                source.ok_or("Select an animation")?
            } else {
                AnimationClip {
                    name: "Animation".into(),
                    duration: 1.,
                    looping: true,
                    tracks: vec![],
                    stride_distance: None,
                }
            };
            let base = clip.name.clone();
            let mut n = 1;
            loop {
                clip.name = format!("{base} {n}");
                if !c
                    .animations
                    .iter()
                    .any(|v| v.name.eq_ignore_ascii_case(&clip.name))
                {
                    break;
                }
                n += 1;
            }
            c.animations.push(clip);
            let index = c.animations.len() - 1;
            e.anim = AnimState {
                clip: Some(index),
                ..default()
            };
            e.view_mut().sel = Sel::Clip(index);
        }
        AnimOp::Delete => {
            let index = e.anim.clip.ok_or("Select an animation")?;
            let c = e.active_character_mut().ok_or("Open a character")?;
            if index < c.animations.len() {
                c.animations.remove(index);
            }
            e.anim = AnimState::default();
            e.view_mut().sel = Sel::None;
        }
        AnimOp::Presets => {
            let body = e.preview_body().ok_or("The character needs a body")?;
            let clips = locomotion_clips(body);
            if clips.is_empty() {
                return Err("Locomotion needs humanoid bones named Torso, Head and Left/Right upper/lower arm/leg".into());
            }
            let c = e.active_character_mut().ok_or("Open a character")?;
            let before = c.animations.len();
            for clip in clips {
                if !c
                    .animations
                    .iter()
                    .any(|v| v.name.eq_ignore_ascii_case(&clip.name))
                {
                    c.animations.push(clip);
                }
            }
            let added = c.animations.len() - before;
            e.notify(
                NoticeKind::Success,
                format!("Added {added} locomotion clip(s)"),
            );
        }
        AnimOp::Play => {
            let clip = e.animation_clip().ok_or("Select an animation")?;
            if !e.anim.playing && !clip.looping && e.anim.time >= clip.duration - 0.001 {
                e.anim.time = 0.;
            }
            e.anim.playing = !e.anim.playing;
        }
        AnimOp::Loop => {
            let clip = clip_mut(e)?;
            clip.looping = !clip.looping;
        }
        AnimOp::Key => {
            let values = e.animation_values().ok_or("Select a bone in the outline")?;
            write_key(e, |key| {
                let (x, y, z) = values.rotation.to_euler(EulerRot::XYZ);
                key.rotation = [x, y, z];
                key.translation = values.translation.to_array();
            })?;
        }
        AnimOp::DeleteKey => {
            let bone = e
                .selected_anim_bone()
                .ok_or("Select a bone in the outline")?;
            let time = e.anim.time;
            let clip = clip_mut(e)?;
            let track = clip
                .tracks
                .iter_mut()
                .find(|t| t.bone == bone)
                .ok_or("No key at the playhead")?;
            let before = track.keys.len();
            track.keys.retain(|k| (k.time - time).abs() >= 0.0001);
            if track.keys.len() == before {
                return Err("No key at the playhead".into());
            }
            clip.tracks.retain(|t| !t.keys.is_empty());
        }
        AnimOp::Step(delta) => {
            let duration = e.animation_clip().ok_or("Select an animation")?.duration;
            e.anim.time = (e.anim.time + delta).clamp(0., duration);
            e.anim.playing = false;
        }
        AnimOp::PrevKey | AnimOp::NextKey => {
            let duration = e.animation_clip().ok_or("Select an animation")?.duration;
            let time = e.anim.time;
            let keys = e.key_times();
            e.anim.time = if op == AnimOp::PrevKey {
                keys.into_iter()
                    .rev()
                    .find(|k| *k < time - 0.001)
                    .unwrap_or(0.)
            } else {
                keys.into_iter()
                    .find(|k| *k > time + 0.001)
                    .unwrap_or(duration)
            };
            e.anim.playing = false;
        }
        AnimOp::Start => {
            e.anim.time = 0.;
            e.anim.playing = false;
        }
        AnimOp::End => {
            e.anim.time = e.animation_clip().ok_or("Select an animation")?.duration;
            e.anim.playing = false;
        }
        AnimOp::BlendWith(target) => {
            e.anim.blend_target = target;
            if target.is_some() && e.anim.blend == 0. {
                e.anim.blend = 0.5;
            }
        }
        AnimOp::Close => {
            e.anim = AnimState::default();
            if matches!(e.sel(), Sel::Clip(_)) {
                e.view_mut().sel = Sel::None;
            }
        }
    }
    Ok(())
}

fn write_key(e: &mut Editor, edit: impl FnOnce(&mut AnimationKey)) -> Result<(), String> {
    let bone = e
        .selected_anim_bone()
        .ok_or("Select a bone in the outline")?;
    let values = e.animation_values().unwrap_or_default();
    let time = e.anim.time;
    e.anim.playing = false;
    let clip = clip_mut(e)?;
    let index = clip
        .tracks
        .iter()
        .position(|t| t.bone == bone)
        .unwrap_or_else(|| {
            clip.tracks.push(BoneTrack { bone, keys: vec![] });
            clip.tracks.len() - 1
        });
    let track = &mut clip.tracks[index];
    let key = match track
        .keys
        .iter()
        .position(|k| (k.time - time).abs() < 0.0001)
    {
        Some(key) => key,
        None => {
            let (x, y, z) = values.rotation.to_euler(EulerRot::XYZ);
            track.keys.push(AnimationKey {
                time,
                rotation: [x, y, z],
                translation: values.translation.to_array(),
            });
            track.keys.len() - 1
        }
    };
    edit(&mut track.keys[key]);
    track.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
    Ok(())
}

/// Move keys at time `from` to `to` on one bone's track, or on every track.
pub(super) fn retime(e: &mut Editor, bone: Option<u32>, from: f32, to: f32) -> Result<(), String> {
    let clip = clip_mut(e)?;
    let to = to.clamp(0., clip.duration);
    let mut moved = 0;
    for track in &mut clip.tracks {
        if bone.is_some_and(|b| b != track.bone) {
            continue;
        }
        let Some(index) = track
            .keys
            .iter()
            .position(|k| (k.time - from).abs() < 0.0005)
        else {
            continue;
        };
        if track
            .keys
            .iter()
            .enumerate()
            .any(|(i, k)| i != index && (k.time - to).abs() < 0.0005)
        {
            return Err("Another key is already at that time".into());
        }
        track.keys[index].time = to;
        track.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        moved += 1;
    }
    if moved == 0 {
        return Err("No key at that time".into());
    }
    e.anim.time = to;
    e.anim.playing = false;
    Ok(())
}

/// Set one component of the selected bone's key at the playhead, creating it.
pub(super) fn set_key(
    e: &mut Editor,
    axis: usize,
    value: f32,
    translation: bool,
) -> Result<(), String> {
    write_key(e, |key| {
        if translation {
            key.translation[axis] = value.clamp(-10., 10.);
        } else {
            key.rotation[axis] = value;
        }
    })
}

pub(super) fn set_time_field(e: &mut Editor, field: Field, value: f32) -> Result<(), String> {
    let duration = e.animation_clip().ok_or("Select an animation")?.duration;
    if field == Field::Time {
        e.anim.time = value.clamp(0., duration);
        e.anim.playing = false;
        return Ok(());
    }
    let next = value.clamp(0.1, 600.);
    let clip = clip_mut(e)?;
    for track in &mut clip.tracks {
        for key in &mut track.keys {
            key.time = key.time / clip.duration * next;
        }
    }
    clip.duration = next;
    e.anim.time = e.anim.time / duration * next;
    Ok(())
}

/// Advance playback and pose the preview character.
pub fn preview(time: Res<Time>, mut editor: ResMut<Editor>, mut visuals: Query<&mut VisualPose>) {
    if editor.active_kind() != Some(DocKind::Character) {
        return;
    }
    if editor.anim.playing
        && let Some(clip) = editor.animation_clip()
    {
        let (duration, looping) = (clip.duration, clip.looping);
        let e = editor.bypass_change_detection();
        let next = e.anim.time + time.delta_secs();
        if looping {
            e.anim.time = next.rem_euclid(duration);
        } else {
            e.anim.time = next.min(duration);
            if next >= duration {
                e.anim.playing = false;
                e.changed();
            }
        }
    }
    let Some(body) = editor.preview_body() else {
        return;
    };
    let mut clips = vec![];
    let mut primary = editor.animation_clip().cloned();
    let mut secondary = editor
        .anim
        .blend_target
        .and_then(|i| editor.active_character()?.animations.get(i).cloned());
    if !editor.anim.playing {
        for clip in primary.iter_mut().chain(secondary.iter_mut()) {
            clip.looping = false;
        }
    }
    if let Some(clip) = primary.as_ref() {
        let blend = if secondary.is_some() {
            editor.anim.blend
        } else {
            0.
        };
        clips.push((clip, editor.anim.time, 1. - blend));
        if let Some(other) = secondary.as_ref() {
            clips.push((
                other,
                editor.anim.time / clip.duration * other.duration,
                blend,
            ));
        }
    }
    let locals = blend_clips(body, &clips);
    for mut visual in &mut visuals {
        visual.locals = locals.clone();
    }
}
