//! Document-action integration tests complement the real-window input scenario.
use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

#[test]
fn explicit_workspace_opens_the_sample_in_the_editor() {
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
    // Like reopening a project, the first character is shown.
    assert_eq!(editor.active_kind(), Some(DocKind::Character));
    assert!(!editor.dirty());
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
            workspace: None,
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
            Action::New(DocKind::Body),
            Action::AddBone { child: false },
            Action::AddBone { child: true },
            Action::AddMount,
            Action::New(DocKind::Prop),
            Action::PaintCursor { erase: false },
            Action::AddAnchor,
            Action::New(DocKind::Character),
            Action::Attach,
        ] {
            self.apply(action);
            assert_ne!(
                self.editor.notice.kind,
                NoticeKind::Error,
                "{}",
                self.editor.notice.text
            );
        }
        self.editor.library.validate().unwrap();
        assert_eq!(self.editor.library.characters[0].attachments.len(), 1);
    }
    fn body_doc(&self) -> DocRef {
        DocRef {
            kind: DocKind::Body,
            id: self.editor.library.bodies[0].id,
        }
    }
    fn prop_doc(&self) -> DocRef {
        DocRef {
            kind: DocKind::Prop,
            id: self.editor.library.props[0].id,
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn new_documents_open_in_tabs_and_start_renaming() {
    let mut s = Session::new();
    assert!(!s.editor.workspace_ready);
    s.apply(Action::New(DocKind::Body));
    assert!(s.editor.workspace_ready);
    assert_eq!(s.editor.tabs, vec![s.body_doc()]);
    assert_eq!(s.editor.active, Some(s.body_doc()));
    assert_eq!(
        s.editor.editing.as_ref().map(|e| e.target.clone()),
        Some(TextTarget::DocName(s.body_doc()))
    );
    s.editor.editing.as_mut().unwrap().buffer = "Hero".into();
    s.apply(Action::CommitEdit);
    assert_eq!(s.editor.library.bodies[0].name, "Hero");
    // Characters need a body; the body tab stays open beside it.
    s.apply(Action::New(DocKind::Character));
    assert_eq!(s.editor.tabs.len(), 2);
    s.apply(Action::CloseTab(s.editor.active.unwrap()));
    assert_eq!(s.editor.active, Some(s.body_doc()));
}

#[test]
fn scratch_creation_branch_delete_and_undo_restore_assembled_character() {
    let mut s = Session::new();
    s.assemble();
    let assembled = s.editor.library.clone();
    s.apply(Action::Open(s.body_doc()));
    let root = s.editor.library.bodies[0].bones[0].id;
    s.apply(Action::Select(Sel::Bone(root)));
    s.apply(Action::DeleteSelection);
    assert!(s.editor.library.bodies[0].bones.is_empty());
    assert!(s.editor.library.bodies[0].mounts.is_empty());
    assert!(s.editor.library.characters[0].attachments.is_empty());
    assert_eq!(s.editor.sel(), Sel::None);
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
    s.apply(Action::Open(s.body_doc()));
    let mount = s.editor.library.bodies[0].mounts[0].id;
    s.apply(Action::Select(Sel::Mount(mount)));
    s.apply(Action::DeleteSelection);
    assert_eq!(s.editor.library.bodies[0].bones.len(), 2);
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, assembled);
    s.apply(Action::Open(s.prop_doc()));
    let anchor = s.editor.library.props[0].anchors[0].id;
    s.apply(Action::Select(Sel::Anchor(anchor)));
    s.apply(Action::DeleteSelection);
    assert!(!s.editor.library.props[0].blocks.is_empty());
    assert!(s.editor.library.props[0].anchors.is_empty());
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, assembled);
}

#[test]
fn choosing_same_body_preserves_equipment_and_changing_body_is_undoable() {
    let mut s = Session::new();
    s.assemble();
    let assembled = s.editor.library.clone();
    let body = s.editor.library.bodies[0].id;
    s.apply(Action::SetBody(body));
    assert_eq!(s.editor.library, assembled);
    let character = s.editor.active.unwrap();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::Open(character));
    let before = s.editor.library.clone();
    let other = s.editor.library.bodies[1].id;
    s.apply(Action::SetBody(other));
    assert_eq!(s.editor.library.characters[0].body, other);
    assert!(s.editor.library.characters[0].attachments.is_empty());
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, before);
}

#[test]
fn empty_names_are_rejected_and_numeric_fields_edit_the_selection() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::CommitEdit);
    let original = s.editor.library.clone();
    s.apply(Action::Rename(s.body_doc()));
    s.editor.editing.as_mut().unwrap().buffer = "   ".into();
    s.apply(Action::CommitEdit);
    assert_eq!(s.editor.library, original);
    assert!(s.editor.notice.text.contains("Name cannot be empty"));
    s.apply(Action::AddBone { child: false });
    s.apply(Action::SetValue(Field::BoneTip(0), 0.3));
    assert!((s.editor.active_body().unwrap().bones[0].tip[0] - 0.3).abs() < 0.0001);
    s.apply(Action::Nudge(Field::BoneTip(0), 0.1));
    assert!((s.editor.active_body().unwrap().bones[0].tip[0] - 0.4).abs() < 0.0001);
    // Typed values go through the same path.
    s.apply(Action::Edit(TextTarget::Value(Field::BoneRadius)));
    s.editor.editing.as_mut().unwrap().buffer = "0.42".into();
    s.apply(Action::CommitEdit);
    assert!((s.editor.active_body().unwrap().bones[0].radius - 0.42).abs() < 0.0001);
    s.apply(Action::Edit(TextTarget::Value(Field::BoneRadius)));
    s.editor.editing.as_mut().unwrap().buffer = "lots".into();
    s.apply(Action::CommitEdit);
    assert_eq!(s.editor.notice.kind, NoticeKind::Error);
}

#[test]
fn drag_gestures_record_one_undo_step() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::AddBone { child: false });
    let before = s.editor.library.clone();
    s.editor.begin_gesture("Edit radius");
    for step in 1..=10 {
        s.editor
            .gesture_edit(|e| fields::set(e, Field::BoneRadius, 0.2 + step as f32 * 0.01));
    }
    s.editor.end_gesture();
    assert!((s.editor.active_body().unwrap().bones[0].radius - 0.3).abs() < 0.0001);
    assert_eq!(s.editor.undo_label(), Some("Edit radius"));
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, before);
}

#[test]
fn default_skeleton_has_the_requested_humanoid_hierarchy() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::DefaultSkeleton);
    assert_ne!(
        s.editor.notice.kind,
        NoticeKind::Error,
        "{}",
        s.editor.notice.text
    );
    let bones = &s.editor.active_body().unwrap().bones;
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
    assert_eq!(bones[6].parent, None);
    s.editor.library.validate().unwrap();
}

#[test]
fn resources_are_deleted_only_when_unreferenced_and_files_follow_on_save() {
    let mut s = Session::new();
    s.assemble();
    s.apply(Action::Save);
    let body = s.body_doc();
    s.apply(Action::Delete(body));
    assert_eq!(s.editor.notice.kind, NoticeKind::Error);
    assert!(s.editor.notice.text.contains("used by"));
    let character = DocRef {
        kind: DocKind::Character,
        id: s.editor.library.characters[0].id,
    };
    s.apply(Action::Delete(character));
    assert!(s.editor.library.characters.is_empty());
    assert!(!s.editor.tabs.contains(&character));
    s.apply(Action::Delete(body));
    assert!(s.editor.library.bodies.is_empty());
    s.apply(Action::Save);
    let reloaded = game::storage::load_workspace(&s.directory).unwrap();
    assert!(reloaded.bodies.is_empty() && reloaded.characters.is_empty());
    assert_eq!(reloaded.props.len(), 1);
    s.apply(Action::Undo);
    assert_eq!(s.editor.library.bodies.len(), 1);
}

#[test]
fn duplicating_a_body_keeps_ids_unique() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::DefaultSkeleton);
    let root = s.editor.active_body().unwrap().bones[0].id;
    s.apply(Action::Select(Sel::Bone(root)));
    s.apply(Action::AddMount);
    s.apply(Action::Duplicate(s.body_doc()));
    assert_eq!(s.editor.library.bodies.len(), 2);
    s.editor.library.validate().unwrap();
    let copy = &s.editor.library.bodies[1];
    assert_eq!(copy.bones.len(), 10);
    assert_eq!(copy.mounts[0].bone, copy.bones[0].id);
    assert_eq!(s.editor.active.unwrap().id, copy.id);
}

#[test]
fn save_revert_and_corrupt_reload_preserve_editable_work() {
    let mut s = Session::new();
    s.assemble();
    assert!(s.editor.dirty());
    let saved = s.editor.library.clone();
    s.apply(Action::Save);
    assert!(!s.editor.dirty());
    assert_eq!(Editor::new(&s.options).library, saved);
    let attachment = s.editor.library.characters[0].attachments[0].id;
    s.apply(Action::Select(Sel::Attachment(attachment)));
    s.apply(Action::DeleteSelection);
    let changed = s.editor.library.clone();
    assert!(s.editor.dirty());
    assert!(s.editor.doc_dirty(s.editor.active.unwrap()));
    assert!(!s.editor.doc_dirty(s.body_doc()));
    s.apply(Action::Revert);
    assert_eq!(s.editor.library, saved);
    assert!(!s.editor.dirty());
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, changed);
    fs::write(s.directory.join("library.json"), b"{broken").unwrap();
    s.apply(Action::Revert);
    assert_eq!(s.editor.library, changed);
    assert!(s.editor.notice.text.contains("Parse"));
    s.apply(Action::Undo);
    assert_eq!(
        s.editor.library, saved,
        "failed reload must not add undo history"
    );
}

#[test]
fn keyboard_shortcuts_preserve_modifier_order_within_one_frame() {
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
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
    s.apply(Action::New(DocKind::Body));
    let editor = std::mem::replace(&mut s.editor, Editor::new(&s.options));
    let options = std::mem::take(&mut s.options);
    let mut app = App::new();
    app.add_message::<KeyboardInput>()
        .add_message::<KeyboardFocusLost>()
        .insert_resource(editor)
        .insert_resource(options)
        .add_systems(Update, input::keyboard);
    // A new resource starts with its name selected: typing replaces it.
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write_batch([
            event(KeyCode::KeyH, ButtonState::Pressed, Some("Hero")),
            event(KeyCode::Enter, ButtonState::Pressed, None),
        ]);
    app.update();
    let editor = app.world().resource::<Editor>();
    assert_eq!(editor.library.bodies[0].name, "Hero");
    assert!(editor.editing.is_none());
    assert!(editor.dirty());
    chord(&mut app, KeyCode::KeyS, "s");
    app.update();
    assert!(!app.world().resource::<Editor>().dirty());
    assert_eq!(
        game::storage::load(&s.directory.join("library.json"))
            .unwrap()
            .bodies[0]
            .name,
        "Hero"
    );
    let doc = app.world().resource::<Editor>().active.unwrap();
    app.world_mut()
        .resource_mut::<Editor>()
        .begin_edit(TextTarget::DocName(doc));
    chord(&mut app, KeyCode::KeyA, "a");
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(event(KeyCode::Backspace, ButtonState::Pressed, None));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Editor>()
            .editing
            .as_ref()
            .unwrap()
            .buffer,
        ""
    );
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write_batch([
            event(KeyCode::KeyH, ButtonState::Pressed, Some("Hero")),
            event(KeyCode::ControlLeft, ButtonState::Pressed, None),
        ]);
    app.update();
    app.world_mut()
        .resource_mut::<Messages<KeyboardFocusLost>>()
        .write(KeyboardFocusLost);
    app.world_mut()
        .resource_mut::<Messages<KeyboardInput>>()
        .write(event(KeyCode::KeyA, ButtonState::Pressed, Some("a")));
    app.update();
    assert_eq!(
        app.world()
            .resource::<Editor>()
            .editing
            .as_ref()
            .unwrap()
            .buffer,
        "Heroa",
        "focus loss must clear a held Control modifier"
    );
}

#[test]
fn animation_keys_names_undo_roundtrip_and_bone_deletion() {
    let mut s = Session::new();
    s.assemble();
    s.apply(Action::Anim(AnimOp::New));
    assert_eq!(s.editor.anim.clip, Some(0));
    s.apply(Action::Edit(TextTarget::ClipName));
    s.editor.editing.as_mut().unwrap().buffer = "Wave".into();
    s.apply(Action::CommitEdit);
    let bone = s.editor.library.bodies[0].bones[0].id;
    s.apply(Action::Select(Sel::Bone(bone)));
    s.apply(Action::Anim(AnimOp::Key));
    s.apply(Action::SetValue(Field::Time, 0.5));
    s.apply(Action::SetValue(Field::KeyRotation(0), 40.));
    let clip = &s.editor.library.characters[0].animations[0];
    assert_eq!(clip.name, "Wave");
    assert_eq!(clip.tracks[0].keys.len(), 2);
    assert!((clip.tracks[0].keys[1].rotation[0] - 40f32.to_radians()).abs() < 0.001);
    s.apply(Action::SetValue(Field::ClipDuration, 2.));
    assert_eq!(
        s.editor.library.characters[0].animations[0].tracks[0].keys[1].time,
        1.
    );
    s.apply(Action::Anim(AnimOp::PrevKey));
    assert_eq!(s.editor.anim.time, 0.);
    s.apply(Action::Anim(AnimOp::NextKey));
    assert_eq!(s.editor.anim.time, 1.);
    let expected = s.editor.library.clone();
    s.apply(Action::Anim(AnimOp::Duplicate));
    assert_eq!(s.editor.library.characters[0].animations[1].name, "Wave 1");
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, expected);
    s.apply(Action::Save);
    s.apply(Action::Select(Sel::Clip(0)));
    s.apply(Action::DeleteSelection);
    assert!(s.editor.library.characters[0].animations.is_empty());
    s.apply(Action::Revert);
    assert_eq!(s.editor.library, expected);
    s.apply(Action::Open(s.body_doc()));
    s.apply(Action::Select(Sel::Bone(bone)));
    s.apply(Action::DeleteSelection);
    assert!(
        s.editor.library.characters[0].animations[0]
            .tracks
            .is_empty()
    );
    s.apply(Action::Undo);
    assert_eq!(s.editor.library, expected);
}

#[test]
fn custom_socket_profiles_rules_and_voxels_are_undoable() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Tile));
    s.apply(Action::PaintCursor { erase: false });
    let voxels = s.editor.library.tiles[0].voxels.clone();
    assert!(!voxels.is_empty());
    s.apply(Action::Undo);
    assert!(s.editor.library.tiles[0].voxels.is_empty());
    s.apply(Action::Redo);
    assert_eq!(s.editor.library.tiles[0].voxels, voxels);
    s.apply(Action::AddSocket);
    assert_eq!(s.editor.sel(), Sel::Socket(0));
    for (target, value) in [
        (TextTarget::SocketKind, "custom"),
        (TextTarget::SocketProfile, "width-24"),
    ] {
        s.apply(Action::Edit(target));
        s.editor.editing.as_mut().unwrap().buffer = value.into();
        s.apply(Action::CommitEdit);
    }
    s.apply(Action::ToggleRule("another".into(), "custom".into()));
    assert_eq!(s.editor.library.tiles[0].sockets[0].kind, "custom");
    assert_eq!(s.editor.library.tiles[0].sockets[0].profile, "width-24");
    assert!(s.editor.library.socket_rules.allows("custom", "another"));
    s.apply(Action::Undo);
    assert!(!s.editor.library.socket_rules.allows("custom", "another"));
    // Starter types pick their matching profile.
    s.apply(Action::SetSocketKind("road".into()));
    assert_eq!(s.editor.library.tiles[0].sockets[0].profile, "road-128");
}

#[test]
fn worlds_place_tiles_through_sockets_and_offer_only_fitting_options() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_workspace");
    let mut s = Session::new();
    s.editor.library = game::storage::load_workspace(&workspace).unwrap();
    s.apply(Action::New(DocKind::World));
    s.apply(Action::CommitEdit);
    let road = s
        .editor
        .library
        .tiles
        .iter()
        .find(|t| t.name == "Road straight")
        .unwrap()
        .id;
    s.apply(Action::SetPlaceTile(road));
    s.apply(Action::Place);
    let w = s.editor.active_world().unwrap();
    assert_eq!(w.instances.len(), 1);
    let first = w.instances[0].id;
    let open = (0..game::tiles::tile(&s.editor.library.tiles, road)
        .unwrap()
        .sockets
        .len())
        .find(|n| {
            game::tiles::tile(&s.editor.library.tiles, road)
                .unwrap()
                .sockets[*n]
                .kind
                == "road"
        })
        .unwrap();
    s.apply(Action::Select(Sel::PieceSocket(first, open)));
    let options = world::options(&s.editor);
    assert!(!options.is_empty());
    let (tile, turn) = options[0];
    s.apply(Action::PlaceOption { tile, turn });
    let w = s.editor.active_world().unwrap();
    assert_eq!(w.instances.len(), 2);
    assert_eq!(w.connections.len(), 1);
    s.apply(Action::RemovePiece);
    assert_eq!(s.editor.active_world().unwrap().instances.len(), 1);
    s.apply(Action::Undo);
    assert_eq!(s.editor.active_world().unwrap().instances.len(), 2);
}

#[test]
fn reparenting_keeps_the_joint_in_place_and_rejects_cycles() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::DefaultSkeleton);
    let bones = s.editor.active_body().unwrap().bones.clone();
    let (back, head, arm) = (bones[0].id, bones[1].id, bones[2].id);
    let body = s.editor.active_body().unwrap();
    let joint = body.bone_segment(arm).unwrap().0;
    s.apply(Action::Select(Sel::Bone(arm)));
    s.apply(Action::SetParent(Some(head)));
    let body = s.editor.active_body().unwrap();
    assert_eq!(body.bones[2].parent, Some(head));
    let moved = body.bone_segment(arm).unwrap().0;
    assert!(Vec3::from_array(joint).distance(Vec3::from_array(moved)) < 1e-5);
    assert_eq!(s.editor.undo_label(), Some("Change parent"));
    // A bone cannot move under its own descendant.
    s.apply(Action::Select(Sel::Bone(back)));
    let before = s.editor.library.clone();
    s.apply(Action::SetParent(Some(arm)));
    assert_eq!(s.editor.library, before);
    assert_eq!(s.editor.notice.kind, NoticeKind::Error);
}

#[test]
fn mirrored_painting_fills_both_sides_in_one_step() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Prop));
    s.apply(Action::Brush(4));
    s.apply(Action::ToggleMirror);
    s.apply(Action::SetValue(Field::Cursor(0), 4.));
    s.apply(Action::PaintCursor { erase: false });
    let blocks = &s.editor.active_prop().unwrap().blocks;
    let mut xs: Vec<_> = blocks.iter().map(|b| b.min[0]).collect();
    xs.sort();
    assert_eq!(xs, [4, 24]);
    assert_eq!(s.editor.undo_label(), Some("Paint"));
    s.apply(Action::Undo);
    assert!(s.editor.active_prop().unwrap().blocks.is_empty());
    assert_eq!(s.editor.notice.text, "Undid Paint");
}

#[test]
fn a_workspace_that_fails_to_load_stays_closed() {
    let s = Session::new();
    fs::write(s.directory.join("library.json"), b"{broken").unwrap();
    let options = LaunchOptions {
        workspace: Some(s.directory.clone()),
        ..Default::default()
    };
    let mut editor = Editor::new(&options);
    assert!(
        !editor.workspace_ready,
        "an empty library must not be saveable over it"
    );
    assert_eq!(editor.notice.kind, NoticeKind::Error);
    assert!(editor.notice.text.contains("Load failed"));
    editor.apply(Action::Save, &options);
    assert_eq!(
        fs::read(s.directory.join("library.json")).unwrap(),
        b"{broken"
    );
}

#[test]
fn tile_edits_that_break_placed_pieces_name_the_world() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_workspace");
    let mut s = Session::new();
    s.editor.library = game::storage::load_workspace(&workspace).unwrap();
    let floor = s
        .editor
        .library
        .tiles
        .iter()
        .find(|t| t.name == "House floor")
        .unwrap()
        .id;
    s.apply(Action::Open(DocRef {
        kind: DocKind::Tile,
        id: floor,
    }));
    s.apply(Action::Select(Sel::Socket(1)));
    let before = s.editor.library.clone();
    s.apply(Action::SetValue(Field::SocketPosition(0), 40.));
    assert_eq!(s.editor.library, before);
    assert!(
        s.editor
            .notice
            .text
            .contains("would break world Neighbourhood"),
        "{}",
        s.editor.notice.text
    );
}

#[test]
fn mirroring_a_limb_creates_the_opposite_side() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::AddBone { child: false });
    let torso = s.editor.active_body().unwrap().bones[0].id;
    s.apply(Action::AddBone { child: true });
    s.apply(Action::Edit(TextTarget::BoneName));
    s.editor.editing.as_mut().unwrap().buffer = "Left arm".into();
    s.apply(Action::CommitEdit);
    s.apply(Action::SetValue(Field::BoneOffset(0), -0.2));
    s.apply(Action::SetValue(Field::BoneTip(0), -0.5));
    let arm = s.editor.active_body().unwrap().bones[1].id;
    s.apply(Action::AddBone { child: true });
    s.apply(Action::AddMount);
    s.apply(Action::Select(Sel::Bone(arm)));
    s.apply(Action::MirrorBranch);
    s.editor.library.validate().unwrap();
    let body = s.editor.active_body().unwrap();
    assert_eq!(body.bones.len(), 5);
    let copy = &body.bones[3];
    assert_eq!(copy.name, "Right arm");
    assert_eq!(copy.parent, Some(torso));
    assert_eq!(copy.offset[0], 0.2);
    assert_eq!(copy.tip[0], 0.5);
    assert_eq!(body.bones[4].parent, Some(copy.id));
    assert_eq!(body.mounts.len(), 2);
    assert_eq!(body.mounts[1].bone, body.bones[4].id);
    assert_eq!(s.editor.sel(), Sel::Bone(copy.id));
}

#[test]
fn keys_retime_per_bone_or_for_the_whole_pose() {
    let mut s = Session::new();
    s.assemble();
    s.apply(Action::Anim(AnimOp::New));
    let bones: Vec<u32> = s.editor.library.bodies[0]
        .bones
        .iter()
        .map(|b| b.id)
        .collect();
    for bone in &bones {
        s.apply(Action::Select(Sel::Bone(*bone)));
        s.apply(Action::SetValue(Field::Time, 0.5));
        s.apply(Action::Anim(AnimOp::Key));
    }
    s.apply(Action::SetValue(Field::Time, 0.8));
    s.apply(Action::Anim(AnimOp::Key));
    let keys = |s: &Session, bone: u32| -> Vec<f32> {
        s.editor.library.characters[0].animations[0]
            .tracks
            .iter()
            .find(|t| t.bone == bone)
            .unwrap()
            .keys
            .iter()
            .map(|k| k.time)
            .collect()
    };
    animation::retime(&mut s.editor, Some(bones[1]), 0.5, 0.25).unwrap();
    assert_eq!(keys(&s, bones[1]), [0.25, 0.8]);
    assert_eq!(keys(&s, bones[0]), [0.5]);
    // Colliding with an existing key is refused.
    assert!(animation::retime(&mut s.editor, Some(bones[1]), 0.25, 0.8).is_err());
    // The whole pose at 0.5 moves together.
    animation::retime(&mut s.editor, None, 0.5, 0.6).unwrap();
    assert_eq!(keys(&s, bones[0]), [0.6]);
    assert_eq!(keys(&s, bones[1]), [0.25, 0.8]);
    assert_eq!(s.editor.anim.time, 0.6);
}

#[test]
fn completing_a_planted_trunk_grows_the_rest_of_the_tree() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_workspace");
    let mut s = Session::new();
    s.editor.library = game::storage::load_workspace(&workspace).unwrap();
    s.apply(Action::New(DocKind::World));
    s.apply(Action::CommitEdit);
    let id = |s: &Session, name: &str| {
        s.editor
            .library
            .tiles
            .iter()
            .find(|t| t.name == name)
            .unwrap()
            .id
    };
    let (grass, trunk) = (id(&s, "Grass"), id(&s, "Tree trunk"));
    s.apply(Action::SetPlaceTile(grass));
    s.apply(Action::Place);
    let ground = s.editor.active_world().unwrap().instances[0].id;
    let plant = game::tiles::tile(&s.editor.library.tiles, grass)
        .unwrap()
        .sockets
        .iter()
        .position(|s| s.kind == "plant")
        .unwrap();
    s.apply(Action::Select(Sel::PieceSocket(ground, plant)));
    let (_, turn) = *world::options(&s.editor)
        .iter()
        .find(|(tile, _)| *tile == trunk)
        .expect("a trunk fits the plant socket");
    s.apply(Action::PlaceOption { tile: trunk, turn });
    let before = s.editor.active_world().unwrap().instances.len();
    s.apply(Action::CompletePiece);
    assert_ne!(
        s.editor.notice.kind,
        NoticeKind::Error,
        "{}",
        s.editor.notice.text
    );
    let w = s.editor.active_world().unwrap();
    assert!(w.instances.len() > before);
    // Every piece placed by completion has its required sockets connected.
    for i in w.instances.iter().filter(|i| i.tile != grass) {
        let t = game::tiles::tile(&s.editor.library.tiles, i.tile).unwrap();
        for (n, socket) in t.sockets.iter().enumerate() {
            assert!(
                !socket.required
                    || w.connected(game::tiles::Port {
                        instance: i.id,
                        socket: n
                    }),
                "{} {}",
                t.name,
                socket.name
            );
        }
    }
    assert_eq!(s.editor.undo_label(), Some("Complete piece"));
    s.apply(Action::Undo);
    assert_eq!(s.editor.active_world().unwrap().instances.len(), before);
}

#[test]
fn duplicating_selections_keeps_names_and_ids_unique() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::DefaultSkeleton);
    let arm = s.editor.active_body().unwrap().bones[2].id;
    s.apply(Action::Select(Sel::Bone(arm)));
    s.apply(Action::DuplicateSelection);
    let body = s.editor.active_body().unwrap();
    assert_eq!(body.bones.len(), 12);
    assert_eq!(body.bones[10].name, "L Arm Upper copy");
    assert_eq!(body.bones[10].offset, body.bones[2].offset);
    assert_eq!(body.bones[11].parent, Some(body.bones[10].id));
    s.editor.library.validate().unwrap();

    s.apply(Action::New(DocKind::Tile));
    s.apply(Action::AddSocket);
    s.apply(Action::DuplicateSelection);
    s.apply(Action::DuplicateSelection);
    let names: Vec<_> = s
        .editor
        .active_tile()
        .unwrap()
        .sockets
        .iter()
        .map(|s| s.name.clone())
        .collect();
    assert_eq!(names, ["socket-0", "socket-1", "socket-2"]);
    assert_eq!(s.editor.sel(), Sel::Socket(2));
    s.editor.library.validate().unwrap();
}

#[test]
fn unsaved_work_is_offered_for_saving_before_switching_workspaces_or_quitting() {
    let mut s = Session::new();
    s.apply(Action::New(DocKind::Body));
    s.apply(Action::CommitEdit);
    assert!(s.editor.dirty());
    let other = s.directory.join("other");
    s.apply(Action::OpenWorkspace(other.clone()));
    assert_eq!(
        s.editor.modal,
        Some(Modal::Unsaved(Pending::Open(other.clone())))
    );
    s.apply(Action::Resolve(true));
    assert!(s.editor.modal.is_none());
    assert_eq!(s.editor.workspace_dir, other);
    assert_eq!(
        game::storage::load_workspace(&s.directory)
            .unwrap()
            .bodies
            .len(),
        1,
        "Save wrote the first workspace before switching"
    );

    // Closing the window with unsaved work asks; discarding then exits.
    s.apply(Action::New(DocKind::Prop));
    s.apply(Action::CommitEdit);
    let editor = std::mem::replace(&mut s.editor, Editor::new(&s.options));
    let mut app = App::new();
    app.add_message::<bevy::window::WindowCloseRequested>()
        .add_message::<AppExit>()
        .insert_resource(editor)
        .add_systems(Update, close_requests);
    app.world_mut()
        .resource_mut::<Messages<bevy::window::WindowCloseRequested>>()
        .write(bevy::window::WindowCloseRequested {
            window: Entity::PLACEHOLDER,
        });
    app.update();
    assert_eq!(
        app.world().resource::<Editor>().modal,
        Some(Modal::Unsaved(Pending::Quit))
    );
    assert!(app.world().resource::<Messages<AppExit>>().is_empty());
    app.world_mut()
        .resource_mut::<Editor>()
        .apply(Action::Resolve(false), &s.options);
    app.update();
    assert!(!app.world().resource::<Messages<AppExit>>().is_empty());
}

#[test]
fn moving_an_equipped_prop_handle_sets_its_mount_space_offset() {
    let mut s = Session::new();
    s.assemble();
    // Rotate the mount so world and mount axes differ.
    let body = s.body_doc();
    s.apply(Action::Open(body));
    let mount = s.editor.library.bodies[0].mounts[0].id;
    s.apply(Action::Select(Sel::Mount(mount)));
    s.apply(Action::SetValue(Field::MountRotation(2), 90.));
    let character = DocRef {
        kind: DocKind::Character,
        id: s.editor.library.characters[0].id,
    };
    s.apply(Action::Open(character));
    let attachment = s.editor.library.characters[0].attachments[0].id;
    s.apply(Action::Select(Sel::Attachment(attachment)));
    let (handle, start, _) = gizmo::handles(&s.editor)[0];
    assert_eq!(handle, gizmo::Handle::Attachment(attachment));
    // Moving up in the world is sideways in the mount's rotated frame.
    s.editor.begin_gesture("Move prop");
    s.editor
        .gesture_edit(|e| gizmo::set(e, handle, start + Vec3::Y * 0.3, false));
    s.editor.end_gesture();
    let offset = Vec3::from_array(s.editor.library.characters[0].attachments[0].offset);
    assert!(
        (offset - Vec3::new(0.3, 0., 0.)).length() < 0.011,
        "{offset}"
    );
    let (_, moved, _) = gizmo::handles(&s.editor)[0];
    assert!((moved - (start + Vec3::Y * 0.3)).length() < 0.011);
}
