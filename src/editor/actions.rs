//! Everything the user can do. Actions are plain data so the UI, keyboard and
//! tests drive the editor identically; the verification manifest records each
//! control's action by its `Debug` form.
use super::*;
use game::tiles::{self, Bounds, Socket, Tile, World, identity, yaw};
use game::voxel;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    // Workspace
    Save,
    Undo,
    Redo,
    Revert,
    Play,
    ShowWorkspaceBrowser,
    Browse(PathBuf),
    OpenWorkspace(PathBuf),
    CloseModal,
    ShowShortcuts,
    Quit,
    /// Answer the unsaved-changes dialog: save first (`true`) or discard.
    Resolve(bool),
    // Explorer and tabs
    ToggleFolder(String),
    ClearFilter,
    New(DocKind),
    Open(DocRef),
    CloseTab(DocRef),
    CloseOtherTabs(DocRef),
    CloseAllTabs,
    Duplicate(DocRef),
    Delete(DocRef),
    Rename(DocRef),
    OpenMenu(Menu),
    CloseMenu,
    // Generic editing
    Select(Sel),
    DeleteSelection,
    DuplicateSelection,
    Edit(TextTarget),
    CommitEdit,
    CancelEdit,
    Nudge(Field, f32),
    SetValue(Field, f32),
    SetTool(Tool),
    Brush(u32),
    Color(usize),
    // Bodies
    AddBone {
        child: bool,
    },
    DefaultSkeleton,
    ToggleShape,
    SetParent(Option<u32>),
    /// Copy the selected bone's branch mirrored across X (Left ↔ Right).
    MirrorBranch,
    AddMount,
    // Props and tiles
    PaintCursor {
        erase: bool,
    },
    ToggleMirror,
    AddAnchor,
    // Characters
    SetBody(u32),
    EquipProp(u32),
    EquipAnchor(u32),
    EquipMount(u32),
    Attach,
    Anim(AnimOp),
    // Tiles
    AddSocket,
    TurnSocket,
    ToggleRequired,
    SetSocketKind(String),
    AddCollider,
    TurnPreview,
    // Worlds
    SetPlaceTile(u32),
    TurnPlacement,
    Place,
    PlaceOption {
        tile: u32,
        turn: usize,
    },
    RemovePiece,
    CompletePiece,
    Validate,
    Generate,
    InstallStarterKit,
    // Socket rules
    ToggleRule(String, String),
    // View
    Frame,
    ToggleGuides,
    TogglePanel(Splitter),
    /// Hide both side panels, or bring them back.
    FocusViewport,
    View(ViewPreset),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewPreset {
    Front,
    Side,
    Top,
    Perspective,
}

pub(super) fn perform(
    e: &mut Editor,
    action: Action,
    options: &LaunchOptions,
) -> Result<(), String> {
    let id = e.library.next_id();
    if !e.workspace_ready && matches!(action, Action::Save | Action::Revert | Action::Play) {
        return Err("Open a workspace first".into());
    }
    match action {
        Action::Save => {
            e.save()?;
            e.notify(NoticeKind::Success, "Saved all resources");
        }
        Action::Undo => e.undo()?,
        Action::Redo => e.redo()?,
        Action::Revert => {
            e.revert()?;
            e.notify(
                NoticeKind::Info,
                "Reverted to the saved workspace (undo restores edits)",
            );
        }
        Action::Play => play(e, options)?,
        Action::ShowWorkspaceBrowser => {
            e.modal = Some(Modal::Workspace {
                dir: e
                    .workspace_dir
                    .canonicalize()
                    .unwrap_or(e.workspace_dir.clone()),
            })
        }
        Action::Browse(dir) => {
            if !dir.is_dir() {
                return Err(format!("Not a directory: {}", dir.display()));
            }
            e.modal = Some(Modal::Workspace { dir });
        }
        Action::OpenWorkspace(dir) => {
            if e.dirty() && e.workspace_ready {
                e.modal = Some(Modal::Unsaved(Pending::Open(dir)));
                return Ok(());
            }
            e.open_workspace(dir)?;
            e.open_first();
        }
        Action::Resolve(save) => {
            let Some(Modal::Unsaved(pending)) = e.modal.take() else {
                return Ok(());
            };
            if save {
                e.save()?;
            }
            match pending {
                Pending::Quit => e.quit = true,
                Pending::Open(dir) => {
                    e.open_workspace(dir)?;
                    e.open_first();
                }
            }
        }
        Action::CloseModal => e.modal = None,
        Action::ShowShortcuts => e.modal = Some(Modal::Shortcuts),
        Action::Quit => {
            if e.dirty() {
                e.modal = Some(Modal::Unsaved(Pending::Quit));
            } else {
                e.quit = true;
            }
        }
        Action::ToggleFolder(key) => {
            if !e.collapsed.remove(&key) {
                e.collapsed.insert(key);
            }
        }
        Action::ClearFilter => {
            e.filter.clear();
            if e.editing
                .as_ref()
                .is_some_and(|t| t.target == TextTarget::Filter)
            {
                e.editing = None;
            }
        }
        Action::New(kind) => new_document(e, kind, id)?,
        Action::Open(doc) => e.open(doc),
        Action::CloseTab(doc) => {
            let index = e.tabs.iter().position(|t| *t == doc);
            e.tabs.retain(|t| *t != doc);
            if e.active == Some(doc) {
                e.active = index
                    .and_then(|i| e.tabs.get(i.min(e.tabs.len().saturating_sub(1))))
                    .copied();
                e.anim = AnimState::default();
            }
        }
        Action::CloseOtherTabs(doc) => {
            e.tabs.retain(|t| *t == doc);
            if e.active != Some(doc) {
                e.open(doc);
            }
        }
        Action::CloseAllTabs => {
            e.tabs.clear();
            e.active = None;
            e.anim = AnimState::default();
        }
        Action::Duplicate(doc) => duplicate(e, doc, id)?,
        Action::Delete(doc) => delete(e, doc)?,
        Action::Rename(doc) => {
            e.open(doc);
            e.begin_edit(TextTarget::DocName(doc));
        }
        Action::OpenMenu(menu) => {
            // Normally opened by the input system, which knows the anchor.
            e.menu = Some(OpenMenu {
                menu,
                pos: Vec2::new(300., 120.),
                width: 220.,
            })
        }
        Action::CloseMenu => e.menu = None,
        Action::Select(sel) => select(e, sel),
        Action::DeleteSelection => delete_selection(e)?,
        Action::Edit(target) => e.begin_edit(target),
        Action::CommitEdit => e.commit_edit(),
        Action::CancelEdit => e.editing = None,
        Action::Nudge(field, delta) => {
            let value = fields::get(e, field).ok_or("Nothing selected to edit")?;
            fields::set(e, field, value + delta)?;
        }
        Action::SetValue(field, value) => fields::set(e, field, value)?,
        Action::SetTool(tool) => {
            e.active.ok_or("Open a document first")?;
            e.view_mut().tool = tool;
        }
        Action::Brush(size) => {
            e.brush = size;
            if e.active_kind() == Some(DocKind::Prop) {
                let view = e.view_mut();
                for v in &mut view.cursor {
                    *v = (*v).clamp(0, 32 - size as i32);
                }
            }
        }
        Action::Color(index) => {
            let color = *PALETTE.get(index).ok_or("No such color")?;
            e.color = color;
            if let (Some(DocKind::Body), Sel::Bone(bone)) = (e.active_kind(), e.sel())
                && let Some(b) = e
                    .active_body_mut()
                    .and_then(|b| b.bones.iter_mut().find(|b| b.id == bone))
            {
                b.color = color;
            }
        }
        Action::AddBone { child } => add_bone(e, child, id)?,
        Action::DefaultSkeleton => default_skeleton(e)?,
        Action::ToggleShape => {
            let Sel::Bone(bone) = e.sel() else {
                return Err("Select a bone".into());
            };
            let b = e
                .active_body_mut()
                .and_then(|b| b.bones.iter_mut().find(|b| b.id == bone))
                .ok_or("Select a bone")?;
            b.shape = match b.shape {
                BodyShape::Capsule => BodyShape::Sphere,
                BodyShape::Sphere => BodyShape::Capsule,
            };
        }
        Action::SetParent(parent) => set_parent(e, parent)?,
        Action::MirrorBranch => copy_branch(e, id, true)?,
        Action::DuplicateSelection => duplicate_selection(e, id)?,
        Action::ToggleMirror => e.mirror = !e.mirror,
        Action::AddMount => {
            let Sel::Bone(bone) = e.sel() else {
                return Err("Select a bone to hold the mount".into());
            };
            let body = e.active_body_mut().ok_or("Open a body")?;
            let radius = body
                .bones
                .iter()
                .find(|b| b.id == bone)
                .ok_or("Select a bone")?
                .radius;
            body.mounts.push(Mount {
                id,
                name: format!("Mount {id}"),
                bone,
                offset: [0., radius, 0.],
                rotation: [0.; 3],
            });
            e.view_mut().sel = Sel::Mount(id);
        }
        Action::PaintCursor { erase } => {
            let cursor = e.cursor();
            paint_at(e, cursor, erase)?;
            e.notify(
                NoticeKind::Info,
                format!("{} at {cursor:?}", if erase { "Erased" } else { "Painted" }),
            );
        }
        Action::AddAnchor => {
            let position = e.cursor().map(|v| v as f32 * voxel::VOXEL_SIZE);
            let prop = e.active_prop_mut().ok_or("Open a prop")?;
            prop.anchors.push(Anchor {
                id,
                name: format!("Anchor {id}"),
                position,
                rotation: [0.; 3],
            });
            e.view_mut().sel = Sel::Anchor(id);
        }
        Action::SetBody(body) => {
            if !e.library.bodies.iter().any(|b| b.id == body) {
                return Err("That body no longer exists".into());
            }
            let c = e.active_character_mut().ok_or("Open a character")?;
            if c.body != body {
                c.body = body;
                let cleared = !c.attachments.is_empty() || !c.animations.is_empty();
                c.attachments.clear();
                c.animations.clear();
                e.equip.mount = None;
                if cleared {
                    e.notify(
                        NoticeKind::Info,
                        "Body changed: equipment and animations were cleared (Ctrl+Z restores)",
                    );
                }
            }
        }
        Action::EquipProp(prop) => {
            e.equip.prop = Some(prop);
            e.equip.anchor = e
                .library
                .props
                .iter()
                .find(|p| p.id == prop)
                .and_then(|p| p.anchors.first())
                .map(|a| a.id);
        }
        Action::EquipAnchor(anchor) => e.equip.anchor = Some(anchor),
        Action::EquipMount(mount) => e.equip.mount = Some(mount),
        Action::Attach => attach(e, id)?,
        Action::Anim(op) => animation::perform(e, op)?,
        Action::AddSocket => {
            let cursor = e.cursor();
            let t = e.active_tile_mut().ok_or("Open a tile")?;
            let mut n = t.sockets.len();
            while t.sockets.iter().any(|s| s.name == format!("socket-{n}")) {
                n += 1;
            }
            t.sockets.push(Socket {
                name: format!("socket-{n}"),
                kind: "land".into(),
                profile: "land-128".into(),
                position: cursor,
                rotation: identity(),
                required: false,
            });
            let index = t.sockets.len() - 1;
            e.view_mut().sel = Sel::Socket(index);
        }
        Action::TurnSocket => {
            let s = selected_socket(e)?;
            s.rotation = (Quat::from_array(yaw(1)) * Quat::from_array(s.rotation)).to_array();
        }
        Action::ToggleRequired => {
            let s = selected_socket(e)?;
            s.required = !s.required;
        }
        Action::SetSocketKind(kind) => {
            let s = selected_socket(e)?;
            s.profile = default_profile(&kind).unwrap_or(&s.profile).to_string();
            s.kind = kind;
        }
        Action::AddCollider => {
            let cursor = e.cursor();
            let brush = e.brush as i32;
            let t = e.active_tile_mut().ok_or("Open a tile")?;
            t.colliders.push(Bounds {
                min: cursor.map(|v| v as f32 * tiles::UNIT),
                max: cursor.map(|v| (v + brush) as f32 * tiles::UNIT),
            });
            let index = t.colliders.len() - 1;
            e.view_mut().sel = Sel::Collider(index);
        }
        Action::TurnPreview => {
            e.active.ok_or("Open a document")?;
            e.view_mut().turn += 1;
            e.scene_revision += 1;
        }
        Action::SetPlaceTile(tile) => e.placement.tile = Some(tile),
        Action::TurnPlacement => e.placement.turn += 1,
        Action::Place => {
            let instance = world::candidate(e)?;
            let piece = instance.id;
            world::connect(e, instance)?;
            e.view_mut().sel = Sel::Piece(piece);
        }
        Action::PlaceOption { tile, turn } => {
            e.placement = Placement {
                tile: Some(tile),
                turn,
            };
            e.menu = None;
            perform(e, Action::Place, options)?;
        }
        Action::RemovePiece => {
            let (Sel::Piece(piece) | Sel::PieceSocket(piece, _)) = e.sel() else {
                return Err("Select a piece".into());
            };
            e.active_world_mut().ok_or("Open a world")?.remove(piece);
        }
        Action::CompletePiece => {
            let placed = world::complete_piece(e)?;
            e.notify(
                NoticeKind::Success,
                if placed == 0 {
                    "Every required socket is already connected".to_string()
                } else {
                    format!("Placed {placed} piece(s) to complete it")
                },
            );
        }
        Action::Validate => {
            let w = e.active_world().ok_or("Open a world")?;
            tiles::validate_world(w, &e.library.tiles, &e.library.socket_rules, true)?;
            e.notify(
                NoticeKind::Success,
                "World is complete: sockets, occupancy and walkways are valid",
            );
        }
        Action::Generate => {
            let g = &e.generator;
            let options = game::generation::GenerateOptions {
                seed: g.seed,
                width: g.size,
                depth: g.size,
                houses: g.houses,
                plant_percent: g.plants,
            };
            let mut w = game::generation::generate(
                &e.library.tiles,
                &e.library.socket_rules,
                id,
                &options,
            )?;
            w.name = format!("Generated {}", g.seed);
            e.library.worlds.push(w);
            e.open(DocRef {
                kind: DocKind::World,
                id,
            });
            e.notify(NoticeKind::Success, "Generated a new world");
        }
        Action::InstallStarterKit => {
            game::generation::install(&mut e.library)?;
            e.notify(NoticeKind::Success, "Installed the starter tile kit");
        }
        Action::ToggleRule(a, b) => {
            let rules = &mut e.library.socket_rules;
            if rules.allows(&a, &b) {
                rules
                    .pairs
                    .retain(|p| !((p[0] == a && p[1] == b) || (p[1] == a && p[0] == b)));
            } else {
                rules.pairs.push([a.clone(), b.clone()]);
            }
            e.rule_focus = Some((a, b));
        }
        Action::Frame => viewport::frame(e),
        Action::ToggleGuides => e.guides = !e.guides,
        Action::TogglePanel(which) => match which {
            Splitter::Left | Splitter::Explorer => e.layout.show_left = !e.layout.show_left,
            Splitter::Right => e.layout.show_right = !e.layout.show_right,
        },
        Action::FocusViewport => {
            let show = !(e.layout.show_left || e.layout.show_right);
            e.layout.show_left = show;
            e.layout.show_right = show;
        }
        Action::View(preset) => {
            let cam = e.cam_mut().ok_or("Open a document")?;
            (cam.yaw, cam.pitch) = match preset {
                ViewPreset::Front => (0., 0.),
                ViewPreset::Side => (std::f32::consts::FRAC_PI_2, 0.),
                ViewPreset::Top => (0., 1.5),
                ViewPreset::Perspective => (0.6, 0.35),
            };
        }
    }
    Ok(())
}

impl Editor {
    pub(super) fn open_first(&mut self) {
        let first = DocKind::ALL
            .into_iter()
            .find_map(|kind| self.docs(kind).first().copied());
        if let Some(doc) = first {
            self.open(doc);
        }
    }
}

fn play(e: &mut Editor, _options: &LaunchOptions) -> Result<(), String> {
    if e.library.characters.is_empty() {
        return Err("Create a character before playing".into());
    }
    let world = e.active_world().map(|w| w.id);
    if let Some(w) = e.active_world() {
        tiles::validate_world(w, &e.library.tiles, &e.library.socket_rules, true)?;
    }
    e.save()?;
    let mut command =
        std::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
    command
        .args(["--mode", "game", "--workspace"])
        .arg(&e.workspace_dir);
    if let Some(id) = world {
        command.args(["--world", &id.to_string()]);
    }
    command.spawn().map_err(|e| format!("Launch game: {e}"))?;
    e.notify(NoticeKind::Success, "Saved and launched a play session");
    Ok(())
}

fn new_document(e: &mut Editor, kind: DocKind, id: u32) -> Result<(), String> {
    let l = &mut e.library;
    match kind {
        DocKind::Body => l.bodies.push(Body {
            id,
            name: format!("Body {id}"),
            bones: vec![],
            mounts: vec![],
        }),
        DocKind::Prop => l.props.push(Prop {
            id,
            name: format!("Prop {id}"),
            blocks: vec![],
            anchors: vec![],
        }),
        DocKind::Character => {
            let body = l.bodies.first().ok_or("Create a body first")?.id;
            l.characters.push(Character {
                id,
                name: format!("Character {id}"),
                body,
                attachments: vec![],
                animations: vec![],
            })
        }
        DocKind::Tile => l.tiles.push(Tile {
            id,
            name: format!("Tile {id}"),
            category: "terrain".into(),
            voxels: vec![],
            sockets: vec![],
            colliders: vec![],
            rotations: (0..4).map(yaw).collect(),
        }),
        DocKind::World => {
            let mut w = World::empty(id);
            w.name = format!("World {id}");
            l.worlds.push(w)
        }
        DocKind::Rules => {
            let doc = DocRef {
                kind: DocKind::Rules,
                id: 0,
            };
            if !e.exists(doc) {
                e.library
                    .socket_rules
                    .pairs
                    .push(["land".into(), "land".into()]);
            }
            e.open(doc);
            return Ok(());
        }
    }
    let doc = DocRef { kind, id };
    e.open(doc);
    e.notify(
        NoticeKind::Success,
        format!("Created {} — type to rename", kind.label().to_lowercase()),
    );
    e.begin_edit(TextTarget::DocName(doc));
    Ok(())
}

fn duplicate(e: &mut Editor, doc: DocRef, id: u32) -> Result<(), String> {
    let l = &mut e.library;
    let copy = |name: &str| format!("{name} copy");
    match doc.kind {
        DocKind::Body => {
            let mut v = l
                .bodies
                .iter()
                .find(|v| v.id == doc.id)
                .ok_or("Missing body")?
                .clone();
            // Bone and mount IDs must stay unique across the library.
            let mut next = id + 1;
            let mut map = HashMap::new();
            for b in &mut v.bones {
                map.insert(b.id, next);
                b.id = next;
                next += 1;
            }
            for b in &mut v.bones {
                b.parent = b.parent.map(|p| map[&p]);
            }
            for m in &mut v.mounts {
                m.id = next;
                m.bone = map[&m.bone];
                next += 1;
            }
            v.id = id;
            v.name = copy(&v.name);
            l.bodies.push(v);
        }
        DocKind::Prop => {
            let mut v = l
                .props
                .iter()
                .find(|v| v.id == doc.id)
                .ok_or("Missing prop")?
                .clone();
            for (n, a) in v.anchors.iter_mut().enumerate() {
                a.id = id + 1 + n as u32;
            }
            v.id = id;
            v.name = copy(&v.name);
            l.props.push(v);
        }
        DocKind::Character => {
            let mut v = l
                .characters
                .iter()
                .find(|v| v.id == doc.id)
                .ok_or("Missing character")?
                .clone();
            for (n, a) in v.attachments.iter_mut().enumerate() {
                a.id = id + 1 + n as u32;
            }
            v.id = id;
            v.name = copy(&v.name);
            l.characters.push(v);
        }
        DocKind::Tile => {
            let mut v = l
                .tiles
                .iter()
                .find(|v| v.id == doc.id)
                .ok_or("Missing tile")?
                .clone();
            v.id = id;
            v.name = copy(&v.name);
            l.tiles.push(v);
        }
        DocKind::World => {
            let mut v = l
                .worlds
                .iter()
                .find(|v| v.id == doc.id)
                .ok_or("Missing world")?
                .clone();
            v.id = id;
            v.name = copy(&v.name);
            l.worlds.push(v);
        }
        DocKind::Rules => return Err("Socket rules are shared by the whole workspace".into()),
    }
    e.open(DocRef { kind: doc.kind, id });
    e.notify(NoticeKind::Success, "Duplicated");
    Ok(())
}

fn delete(e: &mut Editor, doc: DocRef) -> Result<(), String> {
    let name = e.doc_name(doc);
    let l = &mut e.library;
    match doc.kind {
        DocKind::Body => {
            if let Some(c) = l.characters.iter().find(|c| c.body == doc.id) {
                return Err(format!("{name} is used by character {}", c.name));
            }
            l.bodies.retain(|v| v.id != doc.id);
        }
        DocKind::Prop => {
            if let Some(c) = l
                .characters
                .iter()
                .find(|c| c.attachments.iter().any(|a| a.prop == doc.id))
            {
                return Err(format!("{name} is equipped by {}", c.name));
            }
            l.props.retain(|v| v.id != doc.id);
        }
        DocKind::Character => l.characters.retain(|v| v.id != doc.id),
        DocKind::Tile => {
            if let Some(w) = l
                .worlds
                .iter()
                .find(|w| w.instances.iter().any(|i| i.tile == doc.id))
            {
                return Err(format!("{name} is placed in world {}", w.name));
            }
            l.tiles.retain(|v| v.id != doc.id);
        }
        DocKind::World => l.worlds.retain(|v| v.id != doc.id),
        DocKind::Rules => return Err("Socket rules cannot be deleted".into()),
    }
    e.notify(
        NoticeKind::Info,
        format!("Deleted {name} (Ctrl+Z restores)"),
    );
    Ok(())
}

fn select(e: &mut Editor, sel: Sel) {
    if e.active.is_none() {
        return;
    }
    e.view_mut().sel = sel;
    match sel {
        Sel::Clip(index) => {
            if e.anim.clip != Some(index) {
                e.anim.clip = Some(index);
                e.anim.time = 0.;
                e.anim.playing = false;
            }
        }
        Sel::PieceSocket(..) => {
            e.placement.tile = None;
        }
        Sel::Mount(mount) if e.active_kind() == Some(DocKind::Character) => {
            e.equip.mount = Some(mount)
        }
        _ => {}
    }
}

fn delete_selection(e: &mut Editor) -> Result<(), String> {
    match e.sel() {
        Sel::None => return Err("Nothing selected".into()),
        Sel::Bone(bone) if e.active_kind() == Some(DocKind::Body) => {
            let body = e.active_body_mut().ok_or("Open a body")?;
            let removed = body.remove_bone(bone);
            let mounts: Vec<_> = body.mounts.iter().map(|m| m.id).collect();
            let body_id = body.id;
            for c in &mut e.library.characters {
                if c.body == body_id {
                    c.attachments.retain(|a| mounts.contains(&a.mount));
                    for clip in &mut c.animations {
                        clip.tracks.retain(|track| !removed.contains(&track.bone));
                    }
                }
            }
            e.notify(
                NoticeKind::Info,
                format!("Deleted {} bone(s) (Ctrl+Z restores)", removed.len()),
            );
        }
        Sel::Mount(mount) => {
            let body = e.active_body_mut().ok_or("Open a body")?;
            body.mounts.retain(|m| m.id != mount);
            for c in &mut e.library.characters {
                c.attachments.retain(|a| a.mount != mount);
            }
        }
        Sel::Anchor(anchor) => {
            let prop = e.active_prop_mut().ok_or("Open a prop")?;
            prop.anchors.retain(|a| a.id != anchor);
            for c in &mut e.library.characters {
                c.attachments.retain(|a| a.anchor != anchor);
            }
        }
        Sel::Attachment(attachment) => {
            let c = e.active_character_mut().ok_or("Open a character")?;
            c.attachments.retain(|a| a.id != attachment);
        }
        Sel::Clip(_) => animation::perform(e, AnimOp::Delete)?,
        Sel::Socket(index) => {
            let t = e.active_tile_mut().ok_or("Open a tile")?;
            if index < t.sockets.len() {
                t.sockets.remove(index);
            }
        }
        Sel::Collider(index) => {
            let t = e.active_tile_mut().ok_or("Open a tile")?;
            if index < t.colliders.len() {
                t.colliders.remove(index);
            }
        }
        Sel::Piece(_) | Sel::PieceSocket(..) => return perform_remove(e),
        Sel::Bone(_) => return Err("Bones are edited in their body".into()),
    }
    e.view_mut().sel = Sel::None;
    Ok(())
}

fn duplicate_selection(e: &mut Editor, id: u32) -> Result<(), String> {
    let copy = |name: &str| format!("{name} copy");
    match (e.active_kind(), e.sel()) {
        (Some(DocKind::Body), Sel::Bone(_)) => return copy_branch(e, id, false),
        (_, Sel::Mount(mount)) => {
            let body = e.active_body_mut().ok_or("Open a body")?;
            let mut m = body
                .mounts
                .iter()
                .find(|m| m.id == mount)
                .ok_or("Missing mount")?
                .clone();
            m.id = id;
            m.name = copy(&m.name);
            body.mounts.push(m);
            e.view_mut().sel = Sel::Mount(id);
        }
        (_, Sel::Anchor(anchor)) => {
            let prop = e.active_prop_mut().ok_or("Open a prop")?;
            let mut a = prop
                .anchors
                .iter()
                .find(|a| a.id == anchor)
                .ok_or("Missing anchor")?
                .clone();
            a.id = id;
            a.name = copy(&a.name);
            prop.anchors.push(a);
            e.view_mut().sel = Sel::Anchor(id);
        }
        (_, Sel::Socket(index)) => {
            let t = e.active_tile_mut().ok_or("Open a tile")?;
            let mut s = t.sockets.get(index).ok_or("Missing socket")?.clone();
            // "wall-1" duplicates to the next free "wall-N".
            let base = match s.name.rsplit_once('-') {
                Some((base, n)) if n.parse::<u32>().is_ok() => base.to_string(),
                _ => s.name.clone(),
            };
            let mut n = 1;
            while t.sockets.iter().any(|o| o.name == s.name) {
                s.name = format!("{base}-{n}");
                n += 1;
            }
            t.sockets.push(s);
            let index = t.sockets.len() - 1;
            e.view_mut().sel = Sel::Socket(index);
        }
        (_, Sel::Collider(index)) => {
            let t = e.active_tile_mut().ok_or("Open a tile")?;
            let b = t.colliders.get(index).ok_or("Missing box")?.clone();
            t.colliders.push(b);
            let index = t.colliders.len() - 1;
            e.view_mut().sel = Sel::Collider(index);
        }
        (_, Sel::Clip(_)) => animation::perform(e, AnimOp::Duplicate)?,
        _ => return Err("Select something that can be duplicated".into()),
    }
    Ok(())
}

fn perform_remove(e: &mut Editor) -> Result<(), String> {
    let (Sel::Piece(piece) | Sel::PieceSocket(piece, _)) = e.sel() else {
        return Err("Select a piece".into());
    };
    e.active_world_mut().ok_or("Open a world")?.remove(piece);
    e.view_mut().sel = Sel::None;
    Ok(())
}

fn add_bone(e: &mut Editor, child: bool, id: u32) -> Result<(), String> {
    let parent = match (child, e.sel()) {
        (true, Sel::Bone(bone)) => Some(bone),
        (true, Sel::Mount(_)) | (true, _) => return Err("Select a parent bone first".into()),
        (false, _) => None,
    };
    let color = e.color;
    let body = e.active_body_mut().ok_or("Open a body")?;
    if body.bones.len() >= MAX_BONES {
        return Err(format!("Maximum {MAX_BONES} bones per body"));
    }
    let parent_radius = parent
        .and_then(|p| body.bones.iter().find(|b| b.id == p))
        .map_or(0.18, |b| b.radius * 0.85);
    body.bones.push(Bone {
        id,
        name: format!("Bone {id}"),
        parent,
        offset: [0.; 3],
        tip: [0., 0.6, 0.],
        radius: (parent_radius * 100.).round() / 100.,
        shape: BodyShape::Capsule,
        color,
    });
    e.view_mut().sel = Sel::Bone(id);
    Ok(())
}

fn default_skeleton(e: &mut Editor) -> Result<(), String> {
    let mut id = e.library.next_id();
    let color = e.color;
    let body = e.active_body_mut().ok_or("Open a body")?;
    if !body.bones.is_empty() {
        return Err("The humanoid template only fills an empty body".into());
    }
    let mut add = |name: &str, parent: Option<u32>, offset: [f32; 3], tip: [f32; 3], radius| {
        body.bones.push(Bone {
            id,
            name: name.into(),
            parent,
            offset,
            tip,
            radius,
            shape: BodyShape::Capsule,
            color,
        });
        id += 1;
        id - 1
    };
    // Hips at 0.95; arms hang from the shoulders and feet rest on the ground.
    let back = add("Back", None, [0., 0.95, 0.], [0., 0.72, 0.], 0.24);
    add("Head", Some(back), [0., 0.08, 0.], [0., 0.3, 0.], 0.22);
    let l = add(
        "L Arm Upper",
        Some(back),
        [-0.24, -0.12, 0.],
        [-0.4, -0.12, 0.],
        0.12,
    );
    add("L Arm Lower", Some(l), [0.; 3], [-0.36, -0.06, 0.], 0.11);
    let r = add(
        "R Arm Upper",
        Some(back),
        [0.24, -0.12, 0.],
        [0.4, -0.12, 0.],
        0.12,
    );
    add("R Arm Lower", Some(r), [0.; 3], [0.36, -0.06, 0.], 0.11);
    let l = add(
        "L Leg Upper",
        None,
        [-0.15, 0.95, 0.],
        [-0.02, -0.42, 0.],
        0.16,
    );
    add("L Leg Lower", Some(l), [0.; 3], [0., -0.38, 0.], 0.14);
    let r = add(
        "R Leg Upper",
        None,
        [0.15, 0.95, 0.],
        [0.02, -0.42, 0.],
        0.16,
    );
    add("R Leg Lower", Some(r), [0.; 3], [0., -0.38, 0.], 0.14);
    e.view_mut().sel = Sel::Bone(back);
    viewport::frame(e);
    Ok(())
}

/// Paint or erase one brush cube in the active prop or tile.
/// Paint or erase one brush cube in the active prop or tile, plus its mirror
/// image across X when mirroring is on.
pub(super) fn paint_at(e: &mut Editor, cell: [i32; 3], erase: bool) -> Result<(), String> {
    let color = (!erase).then_some(e.color);
    let brush = e.brush;
    let mut cells = vec![cell];
    if e.mirror {
        // Props mirror about the grid's center; tiles about their origin.
        let x = match e.active_kind() {
            Some(DocKind::Prop) => 32 - cell[0] - brush as i32,
            _ => -cell[0] - brush as i32,
        };
        if x != cell[0] {
            cells.push([x, cell[1], cell[2]]);
        }
    }
    for cell in cells {
        match e.active_kind() {
            Some(DocKind::Prop) => {
                let prop = e.active_prop_mut().ok_or("Open a prop")?;
                voxel::paint(prop, cell, brush, color)?;
            }
            Some(DocKind::Tile) => {
                let t = e.active_tile_mut().ok_or("Open a tile")?;
                world::paint(t, cell, brush as i32, color);
            }
            _ => return Err("Open a prop or tile to paint".into()),
        }
    }
    Ok(())
}

/// Swap side words in a name: Left/Right, left/right and L/R tokens.
fn mirror_name(name: &str) -> String {
    let swapped: Vec<String> = name
        .split(' ')
        .map(|word| {
            match word {
                "Left" => "Right",
                "Right" => "Left",
                "left" => "right",
                "right" => "left",
                "L" => "R",
                "R" => "L",
                _ => word,
            }
            .to_string()
        })
        .collect();
    let swapped = swapped.join(" ");
    if swapped == name {
        format!("{name} mirror")
    } else {
        swapped
    }
}

/// Copy the selected bone's branch (with its mounts), optionally mirrored
/// across X with Left/Right names swapped.
fn copy_branch(e: &mut Editor, first_id: u32, mirror: bool) -> Result<(), String> {
    let Sel::Bone(root) = e.sel() else {
        return Err("Select the first bone of a branch".into());
    };
    let body = e.active_body_mut().ok_or("Open a body")?;
    // The branch in parent-before-child order.
    let mut branch = vec![root];
    let mut i = 0;
    while i < branch.len() {
        let parent = branch[i];
        branch.extend(
            body.bones
                .iter()
                .filter(|b| b.parent == Some(parent))
                .map(|b| b.id),
        );
        i += 1;
    }
    if body.bones.len() + branch.len() > MAX_BONES {
        return Err(format!("Maximum {MAX_BONES} bones per body"));
    }
    let mut next = first_id;
    let mut map = HashMap::new();
    for id in &branch {
        map.insert(*id, next);
        next += 1;
    }
    let flip = |v: [f32; 3]| if mirror { [-v[0], v[1], v[2]] } else { v };
    let rename = |name: &str| {
        if mirror {
            mirror_name(name)
        } else {
            format!("{name} copy")
        }
    };
    for id in &branch {
        let mut bone = body.bones.iter().find(|b| b.id == *id).unwrap().clone();
        bone.id = map[id];
        bone.name = rename(&bone.name);
        // The branch root keeps its parent; the rest follow their mirrors.
        bone.parent = bone.parent.map(|p| map.get(&p).copied().unwrap_or(p));
        bone.offset = flip(bone.offset);
        bone.tip = flip(bone.tip);
        body.bones.push(bone);
    }
    let mounts: Vec<_> = body
        .mounts
        .iter()
        .filter(|m| map.contains_key(&m.bone))
        .cloned()
        .collect();
    for mut m in mounts {
        m.id = next;
        next += 1;
        m.bone = map[&m.bone];
        m.name = rename(&m.name);
        m.offset = flip(m.offset);
        if mirror {
            // Mirroring across X negates rotation about Y and Z.
            m.rotation = [m.rotation[0], -m.rotation[1], -m.rotation[2]];
        }
        body.mounts.push(m);
    }
    let copy = map[&root];
    e.view_mut().sel = Sel::Bone(copy);
    e.notify(
        NoticeKind::Success,
        format!(
            "{} {} bone(s)",
            if mirror { "Mirrored" } else { "Copied" },
            branch.len()
        ),
    );
    Ok(())
}

/// Move the selected bone under a new parent (or make it a root), keeping its
/// joint where it is.
fn set_parent(e: &mut Editor, parent: Option<u32>) -> Result<(), String> {
    let Sel::Bone(bone) = e.sel() else {
        return Err("Select a bone".into());
    };
    let body = e.active_body_mut().ok_or("Open a body")?;
    if let Some(p) = parent {
        // The new parent may not be the bone itself or one of its descendants.
        let mut current = Some(p);
        while let Some(id) = current {
            if id == bone {
                return Err("A bone cannot be parented to its own branch".into());
            }
            current = body
                .bones
                .iter()
                .find(|b| b.id == id)
                .and_then(|b| b.parent);
        }
    }
    let (start, _) = body.bone_segment(bone).ok_or("Missing bone")?;
    let origin = parent
        .and_then(|p| body.bone_segment(p))
        .map_or(Vec3::ZERO, |(_, end)| Vec3::from_array(end));
    let b = body
        .bones
        .iter_mut()
        .find(|b| b.id == bone)
        .ok_or("Missing bone")?;
    b.parent = parent;
    b.offset = (Vec3::from_array(start) - origin).to_array();
    Ok(())
}

fn attach(e: &mut Editor, id: u32) -> Result<(), String> {
    let body = e.preview_body().ok_or("The character needs a body")?;
    let mount = e
        .equip
        .mount
        .filter(|m| body.mounts.iter().any(|v| v.id == *m))
        .or(body.mounts.first().map(|m| m.id))
        .ok_or("The body has no mounts — add one in the body editor")?;
    let prop = e
        .equip
        .prop
        .and_then(|p| e.library.props.iter().find(|v| v.id == p))
        .or(e.library.props.first())
        .ok_or("Create a prop first")?;
    let anchor = e
        .equip
        .anchor
        .filter(|a| prop.anchors.iter().any(|v| v.id == *a))
        .or(prop.anchors.first().map(|a| a.id))
        .ok_or("The prop has no anchors — add one in the prop editor")?;
    let prop = prop.id;
    let c = e.active_character_mut().ok_or("Open a character")?;
    c.attachments.push(Attachment {
        id,
        prop,
        anchor,
        mount,
        offset: [0.; 3],
        rotation: [0.; 3],
        scale: 1.,
    });
    e.view_mut().sel = Sel::Attachment(id);
    e.notify(NoticeKind::Success, "Prop attached to the mount");
    Ok(())
}

fn selected_socket(e: &mut Editor) -> Result<&mut Socket, String> {
    let Sel::Socket(index) = e.sel() else {
        return Err("Select a socket".into());
    };
    e.active_tile_mut()
        .and_then(|t| t.sockets.get_mut(index))
        .ok_or_else(|| "Select a socket".into())
}

/// A short description of an action for the undo history.
pub fn describe(action: &Action) -> String {
    let text = match action {
        Action::New(kind) => return format!("New {}", kind.label().to_lowercase()),
        Action::Duplicate(_) => "Duplicate",
        Action::Delete(_) => "Delete resource",
        Action::DeleteSelection => "Delete",
        Action::DuplicateSelection => "Duplicate",
        Action::Nudge(field, _) | Action::SetValue(field, _) => {
            return format!("Edit {}", field.name());
        }
        Action::Color(_) => "Color",
        Action::AddBone { child: true } => "Add child bone",
        Action::AddBone { child: false } => "Add root bone",
        Action::DefaultSkeleton => "Humanoid template",
        Action::ToggleShape => "Change shape",
        Action::SetParent(_) => "Change parent",
        Action::MirrorBranch => "Mirror branch",
        Action::AddMount => "Add mount",
        Action::PaintCursor { erase: true } => "Erase",
        Action::PaintCursor { erase: false } => "Paint",
        Action::AddAnchor => "Add anchor",
        Action::SetBody(_) => "Change body",
        Action::Attach => "Attach prop",
        Action::Anim(op) => match op {
            AnimOp::New => "New clip",
            AnimOp::Duplicate => "Duplicate clip",
            AnimOp::Delete => "Delete clip",
            AnimOp::Presets => "Add locomotion",
            AnimOp::Loop => "Toggle loop",
            AnimOp::Key => "Set key",
            AnimOp::DeleteKey => "Delete key",
            _ => "Animation",
        },
        Action::AddSocket => "Add socket",
        Action::TurnSocket => "Turn socket",
        Action::ToggleRequired => "Toggle required",
        Action::SetSocketKind(_) => "Socket type",
        Action::AddCollider => "Add collision box",
        Action::Place | Action::PlaceOption { .. } => "Place tile",
        Action::RemovePiece => "Remove piece",
        Action::CompletePiece => "Complete piece",
        Action::Generate => "Generate world",
        Action::InstallStarterKit => "Install starter kit",
        Action::ToggleRule(a, b) => return format!("Rule {a} ↔ {b}"),
        Action::CommitEdit | Action::Edit(_) | Action::Rename(_) => "Rename",
        _ => "Edit",
    };
    text.into()
}

/// Profiles of the starter kit's socket types; custom types keep theirs.
pub fn default_profile(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "road" => "road-128",
        "land" | "side-land" | "frontage" => "land-128",
        "door" | "path" | "sidewalk" => "path-16",
        "plot-house" | "house-base" => "house-64",
        "plant" | "trunk-base" | "trunk-top" | "leaf-base" => "tree",
        "branch-mount" | "branch-base" => "branch",
        k if k.contains("roof") || k == "wall-slope" || k == "wall-gable" => "roof-64",
        k if k.contains("wall") => "wall-64",
        _ => return None,
    })
}

/// Every socket type known to the workspace, sorted.
pub fn socket_kinds(l: &Library) -> Vec<String> {
    let mut kinds: Vec<_> = l.socket_rules.pairs.iter().flatten().cloned().collect();
    kinds.extend(
        l.tiles
            .iter()
            .flat_map(|t| t.sockets.iter().map(|s| s.kind.clone())),
    );
    kinds.sort();
    kinds.dedup();
    kinds
}
