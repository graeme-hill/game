//! Character animation editing uses normal document undo/save and shared sampling.
use crate::editor::{Editor, Mode};
use bevy::prelude::*;
use game::{
    animation::{
        AnimationClip, AnimationKey, BoneTrack, LocalPose, blend_clips, locomotion_clips,
        sample_track,
    },
    character_render::VisualPose,
};

#[derive(Default)]
pub struct AnimationEditor {
    pub enabled: bool,
    pub selected: usize,
    pub blend_target: usize,
    pub blend: f32,
    pub time: f32,
    pub playing: bool,
    pub translation: bool,
}
#[derive(Clone, Debug)]
pub enum AnimationAction {
    Tools(bool),
    New,
    Duplicate,
    Delete,
    Select(usize),
    SelectBlend(usize),
    Play,
    Time(f32),
    Step(f32),
    Duration(f32),
    Loop,
    Key,
    DeleteKey,
    Adjust(usize, f32),
    Translation(bool),
    Blend(f32),
    Presets,
}
#[derive(Component)]
pub struct AnimationTimeLabel;

impl Editor {
    pub fn animation_clip(&self) -> Option<&AnimationClip> {
        self.character()?.animations.get(self.animation.selected)
    }
    pub fn animation_values(&self) -> LocalPose {
        let Some(bone) = self
            .preview_body()
            .and_then(|body| body.bones.get(self.bone))
        else {
            return LocalPose::default();
        };
        self.animation_clip()
            .and_then(|clip| {
                clip.tracks
                    .iter()
                    .find(|t| t.bone == bone.id)
                    .map(|track| sample_track(track, self.animation.time, clip.duration, false))
            })
            .unwrap_or_default()
    }
    pub fn animation_action(&mut self, action: &AnimationAction) -> Result<(), String> {
        use AnimationAction::*;
        match *action {
            Tools(enabled) => {
                self.animation.enabled = enabled;
                self.animation.playing = false;
                return Ok(());
            }
            Translation(value) => {
                self.animation.translation = value;
                return Ok(());
            }
            Blend(delta) => {
                self.animation.blend = (self.animation.blend + delta).clamp(0., 1.);
                return Ok(());
            }
            Play => {
                self.animation.playing = !self.animation.playing;
                return Ok(());
            }
            Select(index) => {
                self.animation.selected = index;
                self.animation.time = 0.;
                self.animation.playing = false;
                return Ok(());
            }
            SelectBlend(index) => {
                self.animation.blend_target = index;
                return Ok(());
            }
            Presets => {
                let body = self.preview_body().ok_or("Select a character body")?;
                let clips = locomotion_clips(body);
                let c = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .ok_or("Select a character")?;
                for clip in clips {
                    if !c
                        .animations
                        .iter()
                        .any(|c| c.name.eq_ignore_ascii_case(&clip.name))
                    {
                        c.animations.push(clip);
                    }
                }
                return Ok(());
            }
            New | Duplicate => {
                let c = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .ok_or("Select a character")?;
                let mut clip = if matches!(action, Duplicate) {
                    c.animations
                        .get(self.animation.selected)
                        .cloned()
                        .ok_or("Select an animation")?
                } else {
                    AnimationClip {
                        name: "Animation".into(),
                        duration: 1.,
                        looping: true,
                        tracks: vec![],
                    }
                };
                let base = clip.name.clone();
                let mut index = 1;
                loop {
                    clip.name = format!("{base} {index}");
                    if !c
                        .animations
                        .iter()
                        .any(|c| c.name.eq_ignore_ascii_case(&clip.name))
                    {
                        break;
                    }
                    index += 1;
                }
                c.animations.push(clip);
                self.animation.selected = c.animations.len() - 1;
                self.animation.time = 0.;
                self.animation.playing = false;
                return Ok(());
            }
            Delete => {
                let c = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .ok_or("Select a character")?;
                if self.animation.selected < c.animations.len() {
                    c.animations.remove(self.animation.selected);
                }
                self.animation.time = 0.;
                self.animation.playing = false;
                return Ok(());
            }
            _ => {}
        }
        let duration = self.animation_clip().ok_or("Select an animation")?.duration;
        if let Time(time) = *action {
            self.animation.time = time.clamp(0., duration);
            self.animation.playing = false;
            return Ok(());
        }
        if let Step(delta) = *action {
            self.animation.time = (self.animation.time + delta).clamp(0., duration);
            self.animation.playing = false;
            return Ok(());
        }
        let bone = self
            .preview_body()
            .and_then(|b| b.bones.get(self.bone))
            .map(|b| b.id);
        let values = self.animation_values();
        let c = self
            .library
            .characters
            .get_mut(self.character)
            .ok_or("Select a character")?;
        let clip = c
            .animations
            .get_mut(self.animation.selected)
            .ok_or("Select an animation")?;
        match *action {
            Duration(delta) => {
                let next = (clip.duration + delta).clamp(0.1, 3600.);
                for track in &mut clip.tracks {
                    for key in &mut track.keys {
                        key.time = key.time / clip.duration * next;
                    }
                }
                self.animation.time = self.animation.time / clip.duration * next;
                clip.duration = next;
            }
            Loop => clip.looping = !clip.looping,
            Key | Adjust(_, _) | DeleteKey => {
                let bone = bone.ok_or("Select a bone in the Explorer")?;
                self.animation.playing = false;
                let index = clip
                    .tracks
                    .iter()
                    .position(|track| track.bone == bone)
                    .unwrap_or_else(|| {
                        clip.tracks.push(BoneTrack { bone, keys: vec![] });
                        clip.tracks.len() - 1
                    });
                let track = &mut clip.tracks[index];
                let time = self.animation.time;
                let key = track
                    .keys
                    .iter()
                    .position(|key| (key.time - time).abs() < 0.0001);
                if matches!(action, DeleteKey) {
                    if let Some(key) = key {
                        track.keys.remove(key);
                    }
                } else {
                    let index = key.unwrap_or_else(|| {
                        let (x, y, z) = values.rotation.to_euler(EulerRot::XYZ);
                        track.keys.push(AnimationKey {
                            time,
                            rotation: [x, y, z],
                            translation: values.translation.to_array(),
                        });
                        track.keys.len() - 1
                    });
                    if let Adjust(axis, delta) = *action {
                        if self.animation.translation {
                            track.keys[index].translation[axis] += delta;
                        } else {
                            track.keys[index].rotation[axis] += delta;
                        }
                    }
                    track.keys.sort_by(|a, b| a.time.total_cmp(&b.time));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn preview(
    time: Res<Time>,
    mut editor: ResMut<Editor>,
    mut visuals: Query<&mut VisualPose>,
    mut labels: Query<&mut Text, With<AnimationTimeLabel>>,
) {
    if editor.mode != Mode::Characters {
        return;
    }
    if editor.animation.playing
        && editor.animation.enabled
        && let Some(clip) = editor.animation_clip()
    {
        let next = editor.animation.time + time.delta_secs();
        let duration = clip.duration;
        if clip.looping {
            editor.animation.time = next.rem_euclid(duration);
        } else {
            editor.animation.time = next.min(duration);
            if next >= duration {
                editor.animation.playing = false;
            }
        }
    }
    for mut label in &mut labels {
        label.0 = format!(
            "Time {:.2}s{}",
            editor.animation.time,
            if editor.animation.playing {
                "  • Playing"
            } else {
                ""
            }
        );
    }
    let Some(body) = editor.preview_body() else {
        return;
    };
    let mut primary = editor.animation_clip().cloned();
    let mut secondary = editor
        .character()
        .and_then(|c| c.animations.get(editor.animation.blend_target))
        .cloned();
    if !editor.animation.playing {
        for clip in primary.iter_mut().chain(secondary.iter_mut()) {
            clip.looping = false;
        }
    }
    let mut clips = vec![];
    if editor.animation.enabled
        && let Some(clip) = primary.as_ref()
    {
        clips.push((clip, editor.animation.time, 1. - editor.animation.blend));
        if let Some(other) = secondary.as_ref() {
            clips.push((
                other,
                editor.animation.time / clip.duration * other.duration,
                editor.animation.blend,
            ));
        }
    }
    let locals = blend_clips(body, &clips);
    for mut visual in &mut visuals {
        visual.locals = locals.clone();
    }
}
