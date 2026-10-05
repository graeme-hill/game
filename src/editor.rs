use crate::editor_animation::{AnimationAction, AnimationEditor};
use crate::launch::LaunchOptions;
use bevy::{
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
};
use game::{model::*, storage, voxel};
use serde::Serialize;

pub const PALETTE: [[f32; 3]; 8] = [
    [0.16, 0.78, 0.63],
    [0.98, 0.43, 0.16],
    [0.32, 0.48, 0.95],
    [0.85, 0.28, 0.55],
    [0.95, 0.8, 0.26],
    [0.92, 0.9, 0.84],
    [0.12, 0.15, 0.23],
    [0.60, 0.35, 0.84],
];
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Mode {
    Menu,
    Bodies,
    Props,
    Characters,
    Tiles,
    Worlds,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Asset,
    TileCategory,
    SocketName,
    SocketType,
    SocketProfile,
    RuleLeft,
    RuleRight,
    Workspace,
    Bone,
    Mount,
    Anchor,
    Animation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VectorField {
    BoneOffset,
    BoneTip,
    MountOffset,
    MountRotation,
    AnchorPosition,
    AnchorRotation,
    AttachmentOffset,
    AttachmentRotation,
}
#[derive(Clone, Debug)]
pub enum Action {
    World(crate::editor_world::WorldAction),
    Animation(AnimationAction),
    Navigate(Mode),
    OpenBody(usize),
    OpenProp(usize),
    OpenCharacter(usize),
    CloseDocument(OpenDocument),
    New,
    NewResource(Mode),
    ToggleNewMenu,
    UseWorkspace,
    ChooseWorkspace,
    ShowWorkspacePicker,
    CloseWorkspacePicker,
    BrowseDirectory(std::path::PathBuf),
    SelectWorkspace(std::path::PathBuf),
    Previous,
    Next,
    Rename(Target),
    NameCommit,
    NameCancel,
    Save,
    Reload,
    Undo,
    Redo,
    Starter,
    DefaultSkeleton,
    BoneCycle(i32),
    SelectBone(usize),
    SelectMount(usize),
    AddBone(bool),
    DeleteBone,
    Radius(f32),
    Shape,
    BodyTools(bool),
    PropTools(bool),
    SetVector(VectorField),
    Axis(usize, f32),
    Color(usize),
    AddMount,
    MountCycle(i32),
    DeleteMount,
    Brush(i32),
    Cursor(usize, i32),
    Paint(bool),
    PaintMode(bool),
    AddAnchor,
    AnchorCycle(i32),
    DeleteAnchor,
    ChooseBody,
    ChooseProp,
    Attach,
    AttachmentCycle(i32),
    RemoveAttachment,
    Scale(f32),
    Orbit(f32),
    Zoom(f32),
    Frame,
    Guides,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct OpenDocument {
    pub mode: Mode,
    pub index: usize,
}
#[derive(Component)]
pub struct Control {
    pub action: Action,
    pub label: String,
}
#[derive(Resource)]
pub struct Editor {
    pub animation: AnimationEditor,
    pub library: Library,
    pub mode: Mode,
    pub body: usize,
    pub prop: usize,
    pub character: usize,
    pub world_tools: crate::editor_world::WorldTools,
    pub bone: usize,
    pub mount: usize,
    pub anchor: usize,
    pub attachment: usize,
    pub body_mounts: bool,
    pub asset_browser: bool,
    pub prop_anchors: bool,
    pub vector: VectorField,
    pub brush: u32,
    pub cursor: [i32; 3],
    pub palette: usize,
    pub erase: bool,
    pub dirty: bool,
    pub revision: u64,
    pub notice: String,
    pub naming: Option<(Target, String)>,
    pub yaw: f32,
    pub pitch: f32,
    pub pan: Vec3,
    pub distance: f32,
    pub guides: bool,
    pub workspace_ready: bool,
    pub workspace_dir: std::path::PathBuf,
    pub open_documents: Vec<OpenDocument>,
    pub explorer_new_menu: bool,
    pub workspace_picker: bool,
    pub picker_dir: std::path::PathBuf,
    undo: Vec<Library>,
    redo: Vec<Library>,
}
impl Editor {
    pub fn new(options: &LaunchOptions) -> Self {
        let (library, notice) = match storage::load_workspace(options.workspace_dir()) {
            Ok(library) => (
                library,
                format!("Workspace: {}", options.workspace_dir().display()),
            ),
            Err(e) => (Library::default(), format!("Load failed: {e}")),
        };
        let mode = match options.editor_view.as_deref() {
            Some("tiles") => Mode::Tiles,
            Some("worlds") => Mode::Worlds,
            Some("bodies") => Mode::Bodies,
            Some("props") => Mode::Props,
            Some("characters") => Mode::Characters,
            _ if options.scene => Mode::Characters,
            _ => Mode::Menu,
        };
        let has_document = match mode {
            Mode::Bodies => !library.bodies.is_empty(),
            Mode::Props => !library.props.is_empty(),
            Mode::Characters => !library.characters.is_empty(),
            Mode::Tiles | Mode::Worlds | Mode::Menu => false,
        };
        let open_documents = if has_document {
            vec![OpenDocument { mode, index: 0 }]
        } else {
            vec![]
        };
        Self {
            animation: AnimationEditor::default(),
            library,
            mode,
            body: 0,
            prop: 0,
            character: 0,
            world_tools: Default::default(),
            bone: 0,
            mount: 0,
            anchor: 0,
            attachment: 0,
            body_mounts: options.mounts,
            asset_browser: options.browser || (options.editor_view.is_none() && !options.scene),
            prop_anchors: false,
            vector: VectorField::BoneTip,
            brush: 8,
            cursor: [8, 0, 8],
            palette: 0,
            erase: false,
            dirty: false,
            revision: 1,
            notice,
            naming: None,
            yaw: 0.45,
            pitch: if matches!(mode, Mode::Tiles | Mode::Worlds) {
                0.65
            } else {
                -0.12
            },
            pan: Vec3::ZERO,
            distance: match mode {
                Mode::Tiles => 24.,
                Mode::Worlds => 175.,
                Mode::Props => 6.5,
                _ => 5.,
            },
            guides: true,
            workspace_ready: options.workspace.is_some()
                || options.editor_view.is_some()
                || options.scene
                || options.browser,
            workspace_dir: options.workspace_dir().to_path_buf(),
            open_documents,
            explorer_new_menu: false,
            workspace_picker: false,
            picker_dir: std::env::current_dir().unwrap_or_else(|_| options.data_dir.clone()),
            undo: vec![],
            redo: vec![],
        }
    }
    pub fn body(&self) -> Option<&Body> {
        self.library.bodies.get(self.body)
    }
    pub fn prop(&self) -> Option<&Prop> {
        self.library.props.get(self.prop)
    }
    pub fn character(&self) -> Option<&Character> {
        self.library.characters.get(self.character)
    }
    pub fn preview_body(&self) -> Option<&Body> {
        if self.mode == Mode::Characters {
            self.character()
                .and_then(|c| self.library.bodies.iter().find(|b| b.id == c.body))
        } else {
            self.body()
        }
    }
    pub fn clamp(&mut self) {
        self.world_tools.tile = self
            .world_tools
            .tile
            .min(self.library.tiles.len().saturating_sub(1));
        self.world_tools.world = self
            .world_tools
            .world
            .min(self.library.worlds.len().saturating_sub(1));
        self.world_tools.instance = self.world_tools.instance.min(
            self.library
                .worlds
                .get(self.world_tools.world)
                .map_or(0, |w| w.instances.len().saturating_sub(1)),
        );
        self.body = self.body.min(self.library.bodies.len().saturating_sub(1));
        self.prop = self.prop.min(self.library.props.len().saturating_sub(1));
        self.character = self
            .character
            .min(self.library.characters.len().saturating_sub(1));
        self.bone = self
            .bone
            .min(self.body().map_or(0, |b| b.bones.len().saturating_sub(1)));
        self.mount = self.mount.min(
            self.preview_body()
                .map_or(0, |b| b.mounts.len().saturating_sub(1)),
        );
        self.anchor = self
            .anchor
            .min(self.prop().map_or(0, |p| p.anchors.len().saturating_sub(1)));
        self.attachment = self.attachment.min(
            self.character()
                .map_or(0, |c| c.attachments.len().saturating_sub(1)),
        );
    }
    pub fn changed(&mut self) {
        self.revision += 1;
    }
    fn open_document(&mut self, mode: Mode, index: usize) {
        self.mode = mode;
        let document = OpenDocument { mode, index };
        if !self.open_documents.contains(&document) {
            self.open_documents.push(document);
        }
        self.asset_browser = false;
    }
    fn name(&self, target: Target) -> String {
        match target {
            Target::TileCategory => self
                .library
                .tiles
                .get(self.world_tools.tile)
                .map(|t| t.category.clone()),
            Target::SocketName | Target::SocketType | Target::SocketProfile => self
                .library
                .tiles
                .get(self.world_tools.tile)
                .and_then(|t| t.sockets.get(self.world_tools.socket))
                .map(|s| match target {
                    Target::SocketName => s.name.clone(),
                    Target::SocketType => s.kind.clone(),
                    _ => s.profile.clone(),
                }),
            Target::RuleLeft => Some(self.world_tools.rule_left.clone()),
            Target::RuleRight => Some(self.world_tools.rule_right.clone()),
            Target::Animation => self.animation_clip().map(|clip| clip.name.clone()),
            Target::Asset => match self.mode {
                Mode::Bodies => self.body().map(|b| b.name.clone()),
                Mode::Props => self.prop().map(|p| p.name.clone()),
                Mode::Characters => self.character().map(|c| c.name.clone()),
                Mode::Tiles => self
                    .library
                    .tiles
                    .get(self.world_tools.tile)
                    .map(|t| t.name.clone()),
                Mode::Worlds => self
                    .library
                    .worlds
                    .get(self.world_tools.world)
                    .map(|w| w.name.clone()),
                _ => None,
            },
            Target::Workspace => Some(self.workspace_dir.display().to_string()),
            Target::Bone => self
                .body()
                .and_then(|b| b.bones.get(self.bone))
                .map(|b| b.name.clone()),
            Target::Mount => self
                .body()
                .and_then(|b| b.mounts.get(self.mount))
                .map(|m| m.name.clone()),
            Target::Anchor => self
                .prop()
                .and_then(|p| p.anchors.get(self.anchor))
                .map(|a| a.name.clone()),
        }
        .unwrap_or_default()
    }
    fn set_name(&mut self, target: Target, name: String) {
        let slot = match target {
            Target::TileCategory => self
                .library
                .tiles
                .get_mut(self.world_tools.tile)
                .map(|t| &mut t.category),
            Target::SocketName | Target::SocketType | Target::SocketProfile => self
                .library
                .tiles
                .get_mut(self.world_tools.tile)
                .and_then(|t| t.sockets.get_mut(self.world_tools.socket))
                .map(|s| match target {
                    Target::SocketName => &mut s.name,
                    Target::SocketType => &mut s.kind,
                    _ => &mut s.profile,
                }),
            Target::RuleLeft => Some(&mut self.world_tools.rule_left),
            Target::RuleRight => Some(&mut self.world_tools.rule_right),
            Target::Animation => self
                .library
                .characters
                .get_mut(self.character)
                .and_then(|c| c.animations.get_mut(self.animation.selected))
                .map(|clip| &mut clip.name),
            Target::Asset => match self.mode {
                Mode::Tiles => self
                    .library
                    .tiles
                    .get_mut(self.world_tools.tile)
                    .map(|t| &mut t.name),
                Mode::Worlds => self
                    .library
                    .worlds
                    .get_mut(self.world_tools.world)
                    .map(|w| &mut w.name),
                Mode::Bodies => self.library.bodies.get_mut(self.body).map(|b| &mut b.name),
                Mode::Props => self.library.props.get_mut(self.prop).map(|p| &mut p.name),
                Mode::Characters => self
                    .library
                    .characters
                    .get_mut(self.character)
                    .map(|c| &mut c.name),
                _ => None,
            },
            Target::Workspace => None,
            Target::Bone => self
                .library
                .bodies
                .get_mut(self.body)
                .and_then(|b| b.bones.get_mut(self.bone))
                .map(|b| &mut b.name),
            Target::Mount => self
                .library
                .bodies
                .get_mut(self.body)
                .and_then(|b| b.mounts.get_mut(self.mount))
                .map(|m| &mut m.name),
            Target::Anchor => self
                .library
                .props
                .get_mut(self.prop)
                .and_then(|p| p.anchors.get_mut(self.anchor))
                .map(|a| &mut a.name),
        };
        if let Some(slot) = slot {
            *slot = name;
        }
    }
    pub fn apply(&mut self, action: Action, options: &LaunchOptions) {
        let before = self.library.clone();
        let selected_world_id = self
            .library
            .worlds
            .get(self.world_tools.world)
            .map(|w| w.id);
        let selected_tile_id = self.library.tiles.get(self.world_tools.tile).map(|t| t.id);
        let record = !matches!(action, Action::Undo | Action::Redo | Action::Reload);
        self.notice = String::new();
        let result = self.perform(&action, options);
        if let Err(e) = result {
            self.library = before.clone();
            self.notice = e;
        }
        if self.library != before {
            if let Err(e) = self.library.validate() {
                self.library = before.clone();
                self.notice = format!("Edit rejected: {e}");
            } else {
                if record {
                    self.undo.push(before);
                    if self.undo.len() > 64 {
                        self.undo.remove(0);
                    }
                    self.redo.clear();
                }
                self.dirty = true;
            }
        }
        if self.notice.is_empty() {
            self.notice = match action {
                Action::Navigate(_) => {
                    "Right-drag orbits. Middle-drag pans. Wheel zooms. F12 captures.".into()
                }
                _ => "Updated. Ctrl+S saves; Ctrl+Z undoes.".into(),
            };
        }
        if matches!(action, Action::Reload) && self.notice == "Reopened saved library" {
            if let Some(id) = selected_world_id {
                self.world_tools.world = self
                    .library
                    .worlds
                    .iter()
                    .position(|w| w.id == id)
                    .unwrap_or(0);
            }
            if let Some(id) = selected_tile_id {
                self.world_tools.tile = self
                    .library
                    .tiles
                    .iter()
                    .position(|t| t.id == id)
                    .unwrap_or(0);
            }
            self.dirty = false;
        }
        self.clamp();
        let count = self.character().map_or(0, |c| c.animations.len());
        self.animation.selected = self.animation.selected.min(count.saturating_sub(1));
        self.animation.blend_target = self.animation.blend_target.min(count.saturating_sub(1));
        if let Some(clip) = self.animation_clip() {
            self.animation.time = self.animation.time.min(clip.duration);
        }
        self.changed();
        info!(
            "editor_action action={action:?} mode={:?} revision={} notice={}",
            self.mode, self.revision, self.notice
        );
    }
    fn perform(&mut self, a: &Action, options: &LaunchOptions) -> Result<(), String> {
        let id = self.library.next_id();
        match *a {
            Action::World(ref action) => crate::editor_world::action(self, action)?,
            Action::Animation(ref action) => self.animation_action(action)?,
            Action::UseWorkspace => {
                self.workspace_ready = true;
                self.mode = Mode::Bodies;
                self.notice = format!("Opened workspace {}", self.workspace_dir.display());
            }
            Action::ShowWorkspacePicker => {
                self.workspace_picker = true;
                self.picker_dir = self.workspace_dir.clone();
                self.notice = "Choose a workspace directory.".into();
            }
            Action::CloseWorkspacePicker => self.workspace_picker = false,
            Action::BrowseDirectory(ref path) => {
                if !path.is_dir() {
                    return Err(format!("Not a directory: {}", path.display()));
                }
                self.picker_dir = path.clone();
            }
            Action::SelectWorkspace(ref path) => {
                std::fs::create_dir_all(path)
                    .map_err(|e| format!("Open {}: {e}", path.display()))?;
                self.library = storage::load_workspace(path)?;
                self.workspace_dir = path.clone();
                self.workspace_ready = true;
                self.workspace_picker = false;
                self.open_documents.clear();
                self.mode = Mode::Bodies;
                self.notice = format!("Opened workspace {}", self.workspace_dir.display());
            }
            Action::ChooseWorkspace => {
                self.naming = Some((Target::Workspace, self.workspace_dir.display().to_string()));
                self.notice = "Type a workspace directory; Enter opens it.".into();
            }
            Action::ToggleNewMenu => self.explorer_new_menu = !self.explorer_new_menu,
            Action::NewResource(mode) => {
                self.mode = mode;
                self.perform(&Action::New, options)?;
            }
            Action::Navigate(mode) => {
                self.mode = mode;
                self.asset_browser = false;
                self.naming = None;
                self.yaw = 0.45;
                self.pitch = -0.12;
                self.pan = Vec3::ZERO;
                self.distance = match mode {
                    Mode::Tiles => 24.,
                    Mode::Worlds => 175.,
                    Mode::Props => 6.5,
                    _ => 5.,
                };
                if matches!(mode, Mode::Tiles | Mode::Worlds) {
                    self.pitch = 0.65;
                }
                self.vector = match mode {
                    Mode::Props => VectorField::AnchorPosition,
                    Mode::Characters => VectorField::AttachmentOffset,
                    _ => VectorField::BoneTip,
                };
            }
            Action::OpenBody(index) => {
                self.body = index;
                self.open_document(Mode::Bodies, index);
                self.body_mounts = false;
            }
            Action::OpenProp(index) => {
                self.prop = index;
                self.open_document(Mode::Props, index);
                self.prop_anchors = false;
            }
            Action::OpenCharacter(index) => {
                self.character = index;
                self.open_document(Mode::Characters, index);
            }
            Action::CloseDocument(document) => {
                self.open_documents.retain(|open| *open != document);
                let was_active = match document.mode {
                    Mode::Bodies => self.mode == Mode::Bodies && self.body == document.index,
                    Mode::Props => self.mode == Mode::Props && self.prop == document.index,
                    Mode::Characters => {
                        self.mode == Mode::Characters && self.character == document.index
                    }
                    Mode::Tiles | Mode::Worlds | Mode::Menu => false,
                };
                if was_active {
                    if let Some(next) = self.open_documents.last().copied() {
                        self.open_document(next.mode, next.index);
                    } else {
                        self.mode = Mode::Bodies;
                    }
                }
            }
            Action::New => match self.mode {
                Mode::Bodies => {
                    self.library.bodies.push(Body {
                        id,
                        name: format!("Body {id}"),
                        bones: vec![],
                        mounts: vec![],
                    });
                    self.body = self.library.bodies.len() - 1;
                    self.bone = 0;
                    self.mount = 0;
                    self.body_mounts = false;
                    self.asset_browser = false;
                    self.vector = VectorField::BoneTip;
                    self.open_document(Mode::Bodies, self.body);
                }
                Mode::Props => {
                    self.library.props.push(Prop {
                        id,
                        name: format!("Prop {id}"),
                        blocks: vec![],
                        anchors: vec![],
                    });
                    self.prop = self.library.props.len() - 1;
                    self.anchor = 0;
                    self.prop_anchors = false;
                    self.asset_browser = false;
                    self.cursor = [8, 0, 8];
                    self.vector = VectorField::AnchorPosition;
                    self.open_document(Mode::Props, self.prop);
                }
                Mode::Characters => {
                    let body = self.body().ok_or("Create a body first")?.id;
                    self.library.characters.push(Character {
                        id,
                        name: format!("Character {id}"),
                        body,
                        attachments: vec![],
                        animations: vec![],
                    });
                    self.character = self.library.characters.len() - 1;
                    self.attachment = 0;
                    self.asset_browser = false;
                    self.open_document(Mode::Characters, self.character);
                }
                _ => {}
            },
            Action::Previous | Action::Next => {
                let delta = if matches!(a, Action::Next) { 1 } else { -1 };
                match self.mode {
                    Mode::Bodies => self.body = cycle(self.body, self.library.bodies.len(), delta),
                    Mode::Props => self.prop = cycle(self.prop, self.library.props.len(), delta),
                    Mode::Characters => {
                        self.character = cycle(self.character, self.library.characters.len(), delta)
                    }
                    _ => {}
                };
                self.bone = 0;
                self.mount = 0;
                self.anchor = 0;
                self.attachment = 0;
            }
            Action::Rename(t) => {
                self.naming = Some((t, self.name(t)));
                self.notice = "Type name; Enter confirms, Escape cancels.".into();
            }
            Action::NameCommit => {
                if let Some((target, name)) = self.naming.take() {
                    if name.trim().is_empty() {
                        return Err("Name cannot be empty".into());
                    }
                    if target == Target::Workspace {
                        let path = std::path::PathBuf::from(name.trim());
                        std::fs::create_dir_all(&path)
                            .map_err(|e| format!("Open {}: {e}", path.display()))?;
                        self.library = storage::load_workspace(&path)?;
                        self.workspace_dir = path;
                        self.workspace_ready = true;
                        self.workspace_picker = false;
                        self.open_documents.clear();
                        self.mode = Mode::Bodies;
                        self.notice = format!("Opened workspace {}", self.workspace_dir.display());
                    } else {
                        self.set_name(target, name.trim().chars().take(32).collect());
                    }
                }
            }
            Action::NameCancel => {
                self.naming = None;
            }
            Action::Save => {
                storage::save_workspace(&self.workspace_dir, &self.library)?;
                self.dirty = false;
                self.notice = "Saved workspace resources".into();
                info!("library_saved path={}", options.workspace_dir().display());
            }
            Action::Reload => {
                let loaded = storage::load_workspace(&self.workspace_dir)?;
                self.undo.push(self.library.clone());
                self.redo.clear();
                self.library = loaded;
                self.notice = "Reopened saved library".into();
            }
            Action::Undo => {
                if let Some(old) = self.undo.pop() {
                    self.redo.push(self.library.clone());
                    self.library = old;
                } else {
                    return Err("Nothing to undo".into());
                }
            }
            Action::Redo => {
                if let Some(next) = self.redo.pop() {
                    self.undo.push(self.library.clone());
                    self.library = next;
                } else {
                    return Err("Nothing to redo".into());
                }
            }
            Action::Starter => {
                self.add_starter();
            }
            Action::DefaultSkeleton => self.add_default_skeleton()?,
            Action::BoneCycle(d) => {
                self.bone = cycle(self.bone, self.body().map_or(0, |b| b.bones.len()), d)
            }
            Action::SelectBone(index) => {
                let count = self.preview_body().map_or(0, |b| b.bones.len());
                if index >= count {
                    return Err("That bone no longer exists".into());
                }
                self.bone = index;
                self.notice = self
                    .preview_body()
                    .and_then(|b| b.bones.get(index))
                    .map_or("Bone selected".into(), |b| {
                        format!("Selected bone: {}", b.name)
                    });
            }
            Action::AddBone(child) => {
                let body = self
                    .library
                    .bodies
                    .get_mut(self.body)
                    .ok_or("Create a body first")?;
                if body.bones.len() >= 32 {
                    return Err("Maximum 32 bones per body".into());
                }
                let parent = if child {
                    Some(body.bones.get(self.bone).ok_or("Add a root bone first")?.id)
                } else {
                    None
                };
                body.bones.push(Bone {
                    id,
                    name: format!("Bone {id}"),
                    parent,
                    offset: [0.; 3],
                    tip: [0., 0.6, 0.],
                    radius: 0.18,
                    shape: BodyShape::Capsule,
                    color: PALETTE[self.palette],
                });
                self.bone = body.bones.len() - 1;
            }
            Action::DeleteBone => {
                let body = self.library.bodies.get_mut(self.body).ok_or("No body")?;
                let id = body.bones.get(self.bone).ok_or("No bone")?.id;
                let removed = body.remove_bone(id);
                let mounts: Vec<_> = body.mounts.iter().map(|m| m.id).collect();
                let body_id = body.id;
                for c in &mut self.library.characters {
                    if c.body == body_id {
                        c.attachments.retain(|a| mounts.contains(&a.mount));
                        for clip in &mut c.animations {
                            clip.tracks.retain(|track| !removed.contains(&track.bone));
                        }
                    }
                }
            }
            Action::Radius(d) => {
                let b = self
                    .library
                    .bodies
                    .get_mut(self.body)
                    .and_then(|b| b.bones.get_mut(self.bone))
                    .ok_or("Select a bone")?;
                b.radius = (b.radius + d).clamp(0.03, 1.5);
            }
            Action::Shape => {
                let b = self
                    .library
                    .bodies
                    .get_mut(self.body)
                    .and_then(|b| b.bones.get_mut(self.bone))
                    .ok_or("Select a bone")?;
                b.shape = if b.shape == BodyShape::Capsule {
                    BodyShape::Sphere
                } else {
                    BodyShape::Capsule
                };
            }
            Action::BodyTools(mounts) => {
                self.body_mounts = mounts;
                self.vector = if mounts {
                    VectorField::MountOffset
                } else {
                    VectorField::BoneTip
                };
            }
            Action::SelectMount(index) => self.mount = index,
            Action::PropTools(anchors) => {
                self.prop_anchors = anchors;
                self.vector = VectorField::AnchorPosition;
            }
            Action::SetVector(field) => self.vector = field,
            Action::Axis(axis, d) => {
                let v = match self.vector {
                    VectorField::BoneOffset => self
                        .library
                        .bodies
                        .get_mut(self.body)
                        .and_then(|b| b.bones.get_mut(self.bone))
                        .map(|b| &mut b.offset),
                    VectorField::BoneTip => self
                        .library
                        .bodies
                        .get_mut(self.body)
                        .and_then(|b| b.bones.get_mut(self.bone))
                        .map(|b| &mut b.tip),
                    VectorField::MountOffset => self
                        .library
                        .bodies
                        .get_mut(self.body)
                        .and_then(|b| b.mounts.get_mut(self.mount))
                        .map(|m| &mut m.offset),
                    VectorField::MountRotation => self
                        .library
                        .bodies
                        .get_mut(self.body)
                        .and_then(|b| b.mounts.get_mut(self.mount))
                        .map(|m| &mut m.rotation),
                    VectorField::AnchorPosition => self
                        .library
                        .props
                        .get_mut(self.prop)
                        .and_then(|p| p.anchors.get_mut(self.anchor))
                        .map(|a| &mut a.position),
                    VectorField::AnchorRotation => self
                        .library
                        .props
                        .get_mut(self.prop)
                        .and_then(|p| p.anchors.get_mut(self.anchor))
                        .map(|a| &mut a.rotation),
                    VectorField::AttachmentOffset => self
                        .library
                        .characters
                        .get_mut(self.character)
                        .and_then(|c| c.attachments.get_mut(self.attachment))
                        .map(|a| &mut a.offset),
                    VectorField::AttachmentRotation => self
                        .library
                        .characters
                        .get_mut(self.character)
                        .and_then(|c| c.attachments.get_mut(self.attachment))
                        .map(|a| &mut a.rotation),
                }
                .ok_or("Select an item to edit")?;
                v[axis] += d;
            }
            Action::Color(index) => {
                self.palette = index;
                if self.mode == Mode::Bodies
                    && let Some(b) = self
                        .library
                        .bodies
                        .get_mut(self.body)
                        .and_then(|b| b.bones.get_mut(self.bone))
                {
                    b.color = PALETTE[index];
                }
            }
            Action::AddMount => {
                let body = self
                    .library
                    .bodies
                    .get_mut(self.body)
                    .ok_or("Create a body first")?;
                let bone = body.bones.get(self.bone).ok_or("Select a bone first")?;
                body.mounts.push(Mount {
                    id,
                    name: format!("Mount {id}"),
                    bone: bone.id,
                    offset: [0., bone.radius, 0.],
                    rotation: [0.; 3],
                });
                self.mount = body.mounts.len() - 1;
            }
            Action::MountCycle(d) => {
                self.mount = cycle(
                    self.mount,
                    self.preview_body().map_or(0, |b| b.mounts.len()),
                    d,
                )
            }
            Action::DeleteMount => {
                let b = self.library.bodies.get_mut(self.body).ok_or("No body")?;
                if self.mount < b.mounts.len() {
                    let id = b.mounts.remove(self.mount).id;
                    for c in &mut self.library.characters {
                        c.attachments.retain(|a| a.mount != id);
                    }
                }
            }
            Action::Brush(d) => {
                let sizes = [1, 2, 4, 8, 16];
                let i = sizes.iter().position(|s| *s == self.brush).unwrap_or(0);
                self.brush = sizes[cycle(i, 5, d)];
                for v in &mut self.cursor {
                    *v = (*v).clamp(0, 32 - self.brush as i32);
                }
            }
            Action::Cursor(axis, d) => {
                self.cursor[axis] =
                    (self.cursor[axis] + d * self.brush as i32).clamp(0, 32 - self.brush as i32)
            }
            Action::Paint(erase) => {
                let color = if erase {
                    None
                } else {
                    Some(PALETTE[self.palette])
                };
                let p = self
                    .library
                    .props
                    .get_mut(self.prop)
                    .ok_or("Create a prop first")?;
                voxel::paint(p, self.cursor, self.brush, color)?;
                self.notice = format!(
                    "{} at {:?}",
                    if erase { "Erased" } else { "Painted" },
                    self.cursor
                );
            }
            Action::PaintMode(erase) => self.erase = erase,
            Action::AddAnchor => {
                let p = self
                    .library
                    .props
                    .get_mut(self.prop)
                    .ok_or("Create a prop first")?;
                p.anchors.push(Anchor {
                    id,
                    name: format!("Anchor {id}"),
                    position: self.cursor.map(|v| v as f32 * voxel::VOXEL_SIZE),
                    rotation: [0.; 3],
                });
                self.anchor = p.anchors.len() - 1;
            }
            Action::AnchorCycle(d) => {
                self.anchor = cycle(self.anchor, self.prop().map_or(0, |p| p.anchors.len()), d)
            }
            Action::DeleteAnchor => {
                let p = self.library.props.get_mut(self.prop).ok_or("No prop")?;
                if self.anchor < p.anchors.len() {
                    let id = p.anchors.remove(self.anchor).id;
                    for c in &mut self.library.characters {
                        c.attachments.retain(|a| a.anchor != id);
                    }
                }
            }
            Action::ChooseBody => {
                let bodies = &self.library.bodies;
                let c = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .ok_or("Create a character first")?;
                if bodies.is_empty() {
                    return Err("Create a body first".into());
                }
                let i = bodies.iter().position(|b| b.id == c.body).unwrap_or(0);
                let next = bodies[(i + 1) % bodies.len()].id;
                if next != c.body {
                    c.body = next;
                    c.attachments.clear();
                    c.animations.clear();
                    self.mount = 0;
                }
            }
            Action::ChooseProp => {
                self.prop = cycle(self.prop, self.library.props.len(), 1);
                self.anchor = 0;
            }
            Action::Attach => {
                let m = self
                    .preview_body()
                    .and_then(|b| b.mounts.get(self.mount))
                    .ok_or("Create a body mount first")?
                    .id;
                let p = self.prop().ok_or("Create a prop first")?;
                let anchor = p
                    .anchors
                    .get(self.anchor)
                    .ok_or("Create a prop anchor first")?
                    .id;
                let prop = p.id;
                let c = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .ok_or("Create a character first")?;
                c.attachments.push(Attachment {
                    id,
                    prop,
                    anchor,
                    mount: m,
                    offset: [0.; 3],
                    rotation: [0.; 3],
                    scale: 1.,
                });
                self.attachment = c.attachments.len() - 1;
                self.notice = "Prop attached to body mount".into();
            }
            Action::AttachmentCycle(d) => {
                self.attachment = cycle(
                    self.attachment,
                    self.character().map_or(0, |c| c.attachments.len()),
                    d,
                )
            }
            Action::RemoveAttachment => {
                let c = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .ok_or("No character")?;
                if self.attachment < c.attachments.len() {
                    c.attachments.remove(self.attachment);
                }
            }
            Action::Scale(d) => {
                let a = self
                    .library
                    .characters
                    .get_mut(self.character)
                    .and_then(|c| c.attachments.get_mut(self.attachment))
                    .ok_or("Select an attachment")?;
                a.scale = (a.scale + d).clamp(0.1, 4.);
            }
            Action::Orbit(d) => self.yaw += d,
            Action::Zoom(d) => {
                self.distance = (self.distance + d).clamp(
                    2.,
                    if matches!(self.mode, Mode::Tiles | Mode::Worlds) {
                        200.
                    } else {
                        14.
                    },
                )
            }
            Action::Frame => {
                self.yaw = 0.65;
                self.pitch = -0.12;
                self.pan = Vec3::ZERO;
                self.distance = if matches!(self.mode, Mode::Tiles | Mode::Worlds) {
                    self.pitch = 0.65;
                    crate::editor_world::frame_distance(self)
                } else if self.mode == Mode::Props {
                    6.5
                } else if let Some(body) = self.body() {
                    let mut min = [f32::INFINITY; 3];
                    let mut max = [f32::NEG_INFINITY; 3];
                    for bone in &body.bones {
                        if let Some((start, end)) = body.bone_segment(bone.id) {
                            for point in [start, end] {
                                for axis in 0..3 {
                                    min[axis] = min[axis].min(point[axis]);
                                    max[axis] = max[axis].max(point[axis]);
                                }
                            }
                        }
                    }
                    let span = (0..3).map(|axis| max[axis] - min[axis]).fold(0., f32::max);
                    (span * 2.8).clamp(3.5, 14.)
                } else {
                    5.0
                };
            }
            Action::Guides => self.guides = !self.guides,
        }
        Ok(())
    }
    fn add_starter(&mut self) {
        let mut id = self.library.next_id();
        let body_id = id;
        id += 1;
        let torso = id;
        let mut bones = vec![];
        for (name, parent, offset, tip, radius, shape) in [
            (
                "Torso",
                None,
                [0., 0.8, 0.],
                [0., 0.7, 0.],
                0.28,
                BodyShape::Capsule,
            ),
            (
                "Head",
                Some(torso),
                [0., 0.15, 0.],
                [0., 0.3, 0.],
                0.32,
                BodyShape::Sphere,
            ),
            (
                "Left arm",
                Some(torso),
                [-0.22, -0.1, 0.],
                [-0.55, -0.4, 0.],
                0.14,
                BodyShape::Capsule,
            ),
            (
                "Right arm",
                Some(torso),
                [0.22, -0.1, 0.],
                [0.55, -0.4, 0.],
                0.14,
                BodyShape::Capsule,
            ),
            (
                "Left leg",
                Some(torso),
                [-0.18, -0.7, 0.],
                [-0.08, -0.7, 0.],
                0.18,
                BodyShape::Capsule,
            ),
            (
                "Right leg",
                Some(torso),
                [0.18, -0.7, 0.],
                [0.08, -0.7, 0.],
                0.18,
                BodyShape::Capsule,
            ),
        ] {
            bones.push(Bone {
                id,
                name: name.into(),
                parent,
                offset,
                tip,
                radius,
                shape,
                color: PALETTE[self.palette],
            });
            id += 1;
        }
        let head = bones[1].id;
        self.library.bodies.push(Body {
            id: body_id,
            name: "Starter body".into(),
            bones,
            mounts: vec![Mount {
                id,
                name: "Head top".into(),
                bone: head,
                offset: [0., 0.32, 0.],
                rotation: [0.; 3],
            }],
        });
        self.body = self.library.bodies.len() - 1;
        self.bone = 1;
        self.mount = 0;
    }
    fn add_default_skeleton(&mut self) -> Result<(), String> {
        if self.library.bodies.is_empty() {
            let body_id = self.library.next_id();
            self.library.bodies.push(Body {
                id: body_id,
                name: "Default skeleton".into(),
                bones: vec![],
                mounts: vec![],
            });
            self.body = self.library.bodies.len() - 1;
            self.open_document(Mode::Bodies, self.body);
        }
        let mut id = self.library.next_id();
        let color = PALETTE[self.palette];
        let body = self.library.bodies.get_mut(self.body).ok_or("No body")?;
        if !body.bones.is_empty() {
            return Err("Default skeleton only fills an empty body".into());
        }
        let back = id;
        let mut add = |name: &str, parent: Option<u32>, offset: [f32; 3], tip: [f32; 3], radius| {
            let bone = Bone {
                id,
                name: name.into(),
                parent,
                offset,
                tip,
                radius,
                shape: BodyShape::Capsule,
                color,
            };
            id += 1;
            body.bones.push(bone);
            body.bones.last().unwrap().id
        };
        add("Back", None, [0., 0.9, 0.], [0., 0.85, 0.], 0.24);
        add("Head", Some(back), [0., 0., 0.], [0., 0.34, 0.], 0.22);
        let left_upper = add(
            "L Arm Upper",
            Some(back),
            [-0.20, 0.55, 0.],
            [-0.48, -0.08, 0.],
            0.14,
        );
        add(
            "L Arm Lower",
            Some(left_upper),
            [0., 0., 0.],
            [-0.45, -0.05, 0.],
            0.12,
        );
        let right_upper = add(
            "R Arm Upper",
            Some(back),
            [0.20, 0.55, 0.],
            [0.48, -0.08, 0.],
            0.14,
        );
        add(
            "R Arm Lower",
            Some(right_upper),
            [0., 0., 0.],
            [0.45, -0.05, 0.],
            0.12,
        );
        let left_leg = add(
            "L Leg Upper",
            None,
            [-0.18, 0.85, 0.],
            [-0.08, -0.72, 0.],
            0.18,
        );
        add(
            "L Leg Lower",
            Some(left_leg),
            [0., 0., 0.],
            [0., -0.68, 0.],
            0.15,
        );
        let right_leg = add(
            "R Leg Upper",
            None,
            [0.18, 0.85, 0.],
            [0.08, -0.72, 0.],
            0.18,
        );
        add(
            "R Leg Lower",
            Some(right_leg),
            [0., 0., 0.],
            [0., -0.68, 0.],
            0.15,
        );
        self.bone = 0;
        self.mount = 0;
        self.body_mounts = false;
        self.vector = VectorField::BoneTip;
        Ok(())
    }
}
fn cycle(i: usize, len: usize, d: i32) -> usize {
    if len == 0 {
        0
    } else {
        (i as i32 + d).rem_euclid(len as i32) as usize
    }
}

pub fn controls(
    buttons: Query<(&Interaction, &Control), Changed<Interaction>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut editor: ResMut<Editor>,
    options: Res<LaunchOptions>,
) {
    if mouse.just_pressed(MouseButton::Right)
        && editor.workspace_ready
        && windows
            .single()
            .ok()
            .and_then(Window::cursor_position)
            .is_some_and(|cursor| cursor.x <= 227. && cursor.y >= 38.)
    {
        editor.apply(Action::ToggleNewMenu, &options);
        return;
    }
    if mouse.just_pressed(MouseButton::Left) {
        for (interaction, control) in &buttons {
            if *interaction == Interaction::Pressed {
                editor.apply(control.action.clone(), &options);
                break;
            }
        }
    }
}
pub fn keyboard(
    mut events: MessageReader<KeyboardInput>,
    mut focus_lost: MessageReader<bevy::input::keyboard::KeyboardFocusLost>,
    mut control_keys: Local<[bool; 2]>,
    mut editor: ResMut<Editor>,
    options: Res<LaunchOptions>,
) {
    if focus_lost.read().next().is_some() {
        *control_keys = [false; 2];
    }
    // Preserve modifier ordering for shortcuts pressed and released in one frame.
    for event in events.read() {
        match event.key_code {
            KeyCode::ControlLeft => control_keys[0] = event.state == ButtonState::Pressed,
            KeyCode::ControlRight => control_keys[1] = event.state == ButtonState::Pressed,
            _ => {}
        }
        if event.state != ButtonState::Pressed {
            continue;
        }
        let ctrl = control_keys[0] || control_keys[1];
        if editor.naming.is_some() {
            match event.key_code {
                KeyCode::Enter => editor.apply(Action::NameCommit, &options),
                KeyCode::Escape => editor.apply(Action::NameCancel, &options),
                KeyCode::Backspace => {
                    if let Some((_, s)) = &mut editor.naming {
                        s.pop();
                    }
                    editor.changed();
                }
                KeyCode::KeyA if ctrl => {
                    if let Some((_, s)) = &mut editor.naming {
                        s.clear();
                    }
                    editor.changed();
                }
                _ if !ctrl => {
                    if let Some((target, s)) = &mut editor.naming
                        && let Some(text) = &event.text
                    {
                        for c in text.chars().filter(|c| !c.is_control()) {
                            let limit = if *target == Target::Workspace {
                                240
                            } else {
                                32
                            };
                            if s.chars().count() < limit {
                                s.push(c);
                            }
                        }
                    }
                    editor.changed();
                }
                _ => {}
            }
        } else {
            match event.key_code {
                KeyCode::KeyS if ctrl => editor.apply(Action::Save, &options),
                KeyCode::KeyZ if ctrl => editor.apply(Action::Undo, &options),
                KeyCode::KeyY if ctrl => editor.apply(Action::Redo, &options),
                KeyCode::Escape => editor.apply(Action::Navigate(Mode::Menu), &options),
                _ => {}
            }
        }
    }
}
