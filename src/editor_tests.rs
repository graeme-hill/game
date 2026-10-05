//! Document-action integration tests complement the real-window input scenario.
use crate::{
    editor::{Action, Editor, Mode, Target, VectorField},
    launch::LaunchOptions,
};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

#[test]
fn explicit_workspace_opens_the_same_sample_in_the_editor() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_workspace");
    let options = LaunchOptions {
        workspace: Some(workspace.clone()),
        ..Default::default()
    };
    let editor = Editor::new(&options);
    assert!(editor.workspace_ready);
    assert_eq!(editor.workspace_dir, workspace);
    assert_eq!(editor.library.characters[0].name, "Stickman");
    assert_eq!(editor.library.bodies[0].bones.len(), 10);
}
struct Session {
    options: LaunchOptions,
    editor: Editor,
    directory: PathBuf,
}
impl Session {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!(
            "game-editor-test-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).unwrap();
        let options = LaunchOptions {
            data_dir: directory.clone(),
            ..Default::default()
        };
        let editor = Editor::new(&options);
        Self {
            options,
            editor,
            directory,
        }
    }
    fn apply(&mut self, action: Action) {
        self.editor.apply(action, &self.options);
    }
    fn assemble(&mut self) {
        for action in [
            Action::Navigate(Mode::Bodies),
            Action::New,
            Action::AddBone(false),
            Action::AddBone(true),
            Action::AddMount,
            Action::Navigate(Mode::Props),
            Action::New,
            Action::Paint(false),
            Action::AddAnchor,
            Action::Navigate(Mode::Characters),
            Action::New,
            Action::Attach,
        ] {
            self.apply(action);
        }
        self.editor.library.validate().unwrap();
        assert_eq!(self.editor.library.characters[0].attachments.len(), 1);
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn scratch_creation_branch_delete_and_undo_restore_assembled_character() {
    let mut s = Session::new();
    assert!(s.editor.library.bodies.is_empty());
    s.assemble();
    let assembled = s.editor.library.clone();
    s.apply(Action::Navigate(Mode::Bodies));
    s.apply(Action::BoneCycle(-1));
    s.apply(Action::DeleteBone);
    assert!(s.editor.library.bodies[0].bones.is_empty());
    assert!(s.editor.library.bodies[0].mounts.is_empty());
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.editor.library.validate().unwrap();
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, assembled);
    s.apply(Action::Redo);
    assert!(s.editor.library.bodies[0].bones.is_empty());
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, assembled);
}

#[test]
fn deleting_mount_or_anchor_removes_attachments_and_undo_restores_them() {
    let mut s = Session::new();
    s.assemble();
    let assembled = s.editor.library.clone();
    s.apply(Action::Navigate(Mode::Bodies));
    s.apply(Action::DeleteMount);
    assert_eq!(s.editor.library.bodies[0].bones.len(), 2);
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.editor.library.validate().unwrap();
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, assembled);
    s.apply(Action::Navigate(Mode::Props));
    s.apply(Action::DeleteAnchor);
    assert!(!s.editor.library.props[0].blocks.is_empty());
    assert!(s.editor.library.props[0].anchors.is_empty());
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.editor.library.validate().unwrap();
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, assembled);
}

#[test]
fn choosing_same_body_preserves_equipment_and_changing_body_is_undoable() {
    let mut s = Session::new();
    s.assemble();
    let assembled = s.editor.library.clone();
    s.apply(Action::ChooseBody);
    assert_eq!(s.editor.library, assembled);
    s.apply(Action::Navigate(Mode::Bodies));
    s.apply(Action::New);
    s.apply(Action::Navigate(Mode::Characters));
    let before = s.editor.library.clone();
    s.apply(Action::ChooseBody);
    assert_ne!(
        s.editor.library.characters[0].body,
        before.characters[0].body
    );
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, before);
}

#[test]
fn empty_rename_is_rejected_and_new_body_resets_vector_tools() {
    let mut s = Session::new();
    s.apply(Action::Navigate(Mode::Bodies));
    s.apply(Action::New);
    let original = s.editor.library.clone();
    s.apply(Action::Rename(Target::Asset));
    s.editor.naming = Some((Target::Asset, "   ".into()));
    s.apply(Action::NameCommit);
    assert_eq!(s.editor.library, original);
    assert!(s.editor.notice.contains("Name cannot be empty"));
    s.apply(Action::BodyTools(true));
    s.apply(Action::SetVector(VectorField::MountRotation));
    s.apply(Action::New);
    assert!(!s.editor.body_mounts);
    assert_eq!(s.editor.vector, VectorField::BoneTip);
    s.apply(Action::AddBone(false));
    s.apply(Action::Axis(0, 0.3));
    assert!((s.editor.body().unwrap().bones[0].tip[0] - 0.3).abs() < 0.0001);
}

#[test]
fn default_skeleton_has_the_requested_humanoid_hierarchy() {
    let mut s = Session::new();
    s.apply(Action::Navigate(Mode::Bodies));
    s.apply(Action::New);
    s.apply(Action::DefaultSkeleton);
    assert!(!s.editor.notice.contains("rejected"), "{}", s.editor.notice);
    let bones = &s.editor.body().unwrap().bones;
    let names: Vec<_> = bones.iter().map(|bone| bone.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Back",
            "Head",
            "L Arm Upper",
            "L Arm Lower",
            "R Arm Upper",
            "R Arm Lower",
            "L Leg Upper",
            "L Leg Lower",
            "R Leg Upper",
            "R Leg Lower"
        ]
    );
    assert_eq!(bones[1].parent, Some(bones[0].id));
    assert_eq!(bones[3].parent, Some(bones[2].id));
    assert_eq!(bones[5].parent, Some(bones[4].id));
    assert_eq!(bones[7].parent, Some(bones[6].id));
    assert_eq!(bones[9].parent, Some(bones[8].id));
    assert_eq!(bones[6].parent, None);
    assert_eq!(bones[8].parent, None);
    s.editor.library.validate().unwrap();
}

#[test]
fn save_reopen_and_corrupt_reload_preserve_editable_work() {
    let mut s = Session::new();
    s.assemble();
    assert!(s.editor.dirty);
    let saved = s.editor.library.clone();
    s.apply(Action::Save);
    assert!(!s.editor.dirty);
    assert_eq!(Editor::new(&s.options).library, saved);
    s.apply(Action::RemoveAttachment);
    let changed = s.editor.library.clone();
    assert!(s.editor.dirty);
    s.apply(Action::Reload);
    assert_eq!(s.editor.library, saved);
    assert!(!s.editor.dirty);
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, changed);
    fs::write(s.directory.join("library.json"), b"{broken").unwrap();
    s.apply(Action::Reload);
    assert_eq!(s.editor.library, changed);
    assert!(s.editor.notice.contains("Parse"));
    s.apply(Action::Undo);
    assert_eq!(
        s.editor.library, saved,
        "failed reload must not add undo history"
    );
}

#[test]
fn keyboard_shortcuts_preserve_modifier_order_within_one_frame() {
    use bevy::{
        input::{
            ButtonState,
            keyboard::{Key, KeyboardFocusLost, KeyboardInput},
        },
        prelude::*,
    };
    fn event(key_code: KeyCode, state: ButtonState, text: Option<&str>) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key: Key::Character(text.unwrap_or("").into()),
            state,
            text: text.map(Into::into),
            repeat: false,
            window: Entity::PLACEHOLDER,
        }
    }
    fn chord(app: &mut App, key: KeyCode, text: &str) {
        let mut messages = app.world_mut().resource_mut::<Messages<KeyboardInput>>();
        messages.write_batch([
            event(KeyCode::ControlLeft, ButtonState::Pressed, None),
            event(key, ButtonState::Pressed, Some(text)),
            event(key, ButtonState::Released, None),
            event(KeyCode::ControlLeft, ButtonState::Released, None),
        ]);
    }
    let mut s = Session::new();
    s.apply(Action::Navigate(Mode::Bodies));
    s.apply(Action::New);
    s.apply(Action::Rename(Target::Asset));
    let editor = std::mem::replace(&mut s.editor, Editor::new(&s.options));
    let options = std::mem::take(&mut s.options);
    let mut app = App::new();
    app.add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .insert_resource(editor)
        .insert_resource(options)
        .add_systems(Update, crate::editor::keyboard);
    chord(&mut app, KeyCode::KeyA, "a");
    app.update();
    assert_eq!(
        app.world().resource::<Editor>().naming.as_ref().unwrap().1,
        ""
    );
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write_batch([
            event(KeyCode::KeyH, ButtonState::Pressed, Some("Hero")),
            event(KeyCode::Enter, ButtonState::Pressed, None),
        ]);
    app.update();
    let editor = app.world().resource::<Editor>();
    assert_eq!(editor.library.bodies[0].name, "Hero");
    assert!(editor.naming.is_none());
    assert!(editor.dirty);
    chord(&mut app, KeyCode::KeyS, "s");
    app.update();
    assert!(!app.world().resource::<Editor>().dirty);
    assert_eq!(
        game::storage::load(&s.directory.join("library.json"))
            .unwrap()
            .bodies[0]
            .name,
        "Hero"
    );
    app.world_mut().resource_mut::<Editor>().naming = Some((Target::Asset, "Hero".into()));
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(event(KeyCode::ControlLeft, ButtonState::Pressed, None));
    app.update();
    app.world_mut()
        .resource_mut::<Messages<KeyboardFocusLost>>()
        .write(KeyboardFocusLost);
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(event(KeyCode::KeyA, ButtonState::Pressed, Some("a")));
    app.update();
    assert_eq!(
        app.world().resource::<Editor>().naming.as_ref().unwrap().1,
        "Heroa",
        "focus loss must clear a held Control modifier"
    );
}

#[test]
fn animation_keys_names_undo_roundtrip_and_bone_deletion() {
    use crate::editor_animation::AnimationAction as A;
    let mut s = Session::new();
    s.assemble();
    s.apply(Action::Animation(A::New));
    s.apply(Action::Rename(Target::Animation));
    s.editor.naming = Some((Target::Animation, "Wave".into()));
    s.apply(Action::NameCommit);
    s.apply(Action::Animation(A::Key));
    s.apply(Action::Animation(A::Time(0.5)));
    s.apply(Action::Animation(A::Adjust(0, 0.7)));
    let clip = &s.editor.library.characters[0].animations[0];
    assert_eq!(clip.name, "Wave");
    assert_eq!(clip.tracks[0].keys.len(), 2);
    assert_eq!(clip.tracks[0].keys[1].rotation[0], 0.7);
    s.apply(Action::Animation(A::Duration(1.)));
    assert_eq!(
        s.editor.library.characters[0].animations[0].tracks[0].keys[1].time,
        1.
    );
    let expected = s.editor.library.clone();
    s.apply(Action::Animation(A::Duplicate));
    assert_eq!(s.editor.library.characters[0].animations[1].name, "Wave 1");
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, expected);
    s.apply(Action::Save);
    s.apply(Action::Animation(A::Delete));
    s.apply(Action::Reload);
    assert_eq!(s.editor.library, expected);
    s.apply(Action::Navigate(Mode::Bodies));
    s.editor.bone = 0;
    s.apply(Action::DeleteBone);
    assert!(
        s.editor.library.characters[0].animations[0]
            .tracks
            .is_empty()
    );
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, expected);
}
