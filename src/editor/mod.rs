//! Workspace editor: a resource explorer, tabbed documents, a 3D viewport and an
//! inspector. Document state lives in [`Editor`]; UI, viewport and input are
//! separate systems that read it and send [`Action`]s back.
mod actions;
mod animation;
mod fields;
mod gizmo;
mod input;
mod menus;
mod recent;
#[cfg(test)]
mod tests;
mod ui;
mod viewport;
mod world;

pub use actions::Action;
pub use animation::AnimOp;
pub use fields::{Field, TextTarget};
pub use menus::Menu;

use crate::launch::LaunchOptions;
use bevy::prelude::*;
use game::{model::*, storage};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct EditorPlugin;

impl Plugin for EditorPlugin {
    fn build(&self, app: &mut App) {
        app.init_gizmo_group::<viewport::OverlayGuides>()
            .init_resource::<input::EditorPointer>()
            .init_resource::<ui::ScrollMemory>()
            .init_resource::<ui::OutlineOrder>()
            .init_resource::<viewport::ViewportRect>()
            .init_resource::<viewport::HoverCell>()
            .init_resource::<gizmo::GizmoHover>()
            .init_resource::<viewport::ViewportHover>()
            .add_systems(Startup, (viewport::setup, ui::theme::load_fonts))
            .add_systems(
                Update,
                (
                    ui::responsive_scale,
                    input::controls,
                    input::keyboard,
                    input::scroll_panes,
                    viewport::sync_viewport,
                    viewport::camera_input,
                    viewport::pointer_tools,
                    viewport::camera,
                    viewport::rebuild,
                    animation::preview,
                    ui::rebuild_ui,
                    input::cursor_icons,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    viewport::guides,
                    world::guides,
                    ui::hover_styles,
                    ui::live_labels,
                    ui::scrollbars,
                    ui::tooltips,
                    ui::hover_label,
                    close_requests,
                )
                    .after(ui::rebuild_ui),
            )
            .add_systems(
                PostUpdate,
                (ui::reveal_rows, ui::manifest::snapshot).after(bevy::ui::UiSystems::Layout),
            );
    }
}

/// Closing the window with unsaved work asks first; a confirmed quit exits.
fn close_requests(
    mut requests: MessageReader<bevy::window::WindowCloseRequested>,
    mut e: ResMut<Editor>,
    mut exit: MessageWriter<AppExit>,
) {
    if requests.read().count() > 0 {
        if e.dirty() {
            e.commit_edit();
            e.modal = Some(Modal::Unsaved(Pending::Quit));
            e.changed();
        } else {
            e.quit = true;
        }
    }
    if e.quit {
        exit.write(AppExit::Success);
    }
}

/// The kinds of source resources a workspace contains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
pub enum DocKind {
    Body,
    Prop,
    Character,
    Tile,
    World,
    Rules,
}

impl DocKind {
    pub const ALL: [DocKind; 6] = [
        DocKind::Body,
        DocKind::Prop,
        DocKind::Character,
        DocKind::Tile,
        DocKind::World,
        DocKind::Rules,
    ];
    pub fn label(self) -> &'static str {
        match self {
            DocKind::Body => "Body",
            DocKind::Prop => "Prop",
            DocKind::Character => "Character",
            DocKind::Tile => "Tile",
            DocKind::World => "World",
            DocKind::Rules => "Socket rules",
        }
    }
    pub fn plural(self) -> &'static str {
        match self {
            DocKind::Body => "Bodies",
            DocKind::Prop => "Props",
            DocKind::Character => "Characters",
            DocKind::Tile => "Tiles",
            DocKind::World => "Worlds",
            DocKind::Rules => "Rules",
        }
    }
    pub fn extension(self) -> &'static str {
        match self {
            DocKind::Body => "body",
            DocKind::Prop => "prop",
            DocKind::Character => "character",
            DocKind::Tile => "tile",
            DocKind::World => "world",
            DocKind::Rules => "sockets",
        }
    }
    fn from_view(view: &str) -> Option<Self> {
        Some(match view {
            "bodies" => DocKind::Body,
            "props" => DocKind::Prop,
            "characters" => DocKind::Character,
            "tiles" => DocKind::Tile,
            "worlds" => DocKind::World,
            _ => return None,
        })
    }
}

/// A stable reference to a document. Rules are a singleton with id 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
pub struct DocRef {
    pub kind: DocKind,
    pub id: u32,
}

/// The selected item inside the active document.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub enum Sel {
    #[default]
    None,
    Bone(u32),
    Mount(u32),
    Anchor(u32),
    Attachment(u32),
    Clip(usize),
    Socket(usize),
    Collider(usize),
    Piece(u32),
    PieceSocket(u32, usize),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum Tool {
    #[default]
    Select,
    Paint,
    Erase,
    Socket,
    Collider,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Cam {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub pan: [f32; 3],
}

/// Per-document view state, kept while the document's tab is open.
#[derive(Clone, Debug, Serialize)]
pub struct DocView {
    pub sel: Sel,
    pub cam: Cam,
    pub tool: Tool,
    pub cursor: [i32; 3],
    pub turn: usize,
    pub framed: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AnimState {
    pub clip: Option<usize>,
    pub time: f32,
    pub playing: bool,
    pub blend_target: Option<usize>,
    pub blend: f32,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Equip {
    pub prop: Option<u32>,
    pub anchor: Option<u32>,
    pub mount: Option<u32>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GenSettings {
    pub seed: u64,
    pub size: usize,
    pub houses: usize,
    pub plants: u8,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Placement {
    pub tile: Option<u32>,
    pub turn: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum NoticeKind {
    #[default]
    Info,
    Success,
    Error,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Notice {
    pub text: String,
    pub kind: NoticeKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextEdit {
    pub target: TextTarget,
    pub buffer: String,
    /// The whole value is selected: the next character replaces it.
    pub fresh: bool,
    /// Caret position in characters.
    pub caret: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OpenMenu {
    pub menu: Menu,
    /// Logical (pre-UiScale) top-left position.
    pub pos: Vec2,
    pub width: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Modal {
    Workspace {
        dir: PathBuf,
    },
    /// Unsaved changes would be lost by what the user asked for next.
    Unsaved(Pending),
    Shortcuts,
}

/// What to do once unsaved changes have been saved or discarded.
#[derive(Clone, Debug, PartialEq)]
pub enum Pending {
    Quit,
    Open(PathBuf),
}

pub const PALETTE: [[f32; 3]; 24] = [
    [0.95, 0.95, 0.93],
    [0.70, 0.71, 0.73],
    [0.42, 0.43, 0.46],
    [0.20, 0.21, 0.24],
    [0.10, 0.10, 0.12],
    [0.55, 0.36, 0.22],
    [0.90, 0.30, 0.30],
    [0.98, 0.43, 0.16],
    [0.95, 0.65, 0.20],
    [0.95, 0.85, 0.30],
    [0.70, 0.85, 0.30],
    [0.35, 0.75, 0.35],
    [0.16, 0.78, 0.63],
    [0.20, 0.70, 0.80],
    [0.32, 0.48, 0.95],
    [0.25, 0.30, 0.70],
    [0.60, 0.35, 0.84],
    [0.85, 0.28, 0.55],
    [0.98, 0.65, 0.75],
    [0.98, 0.85, 0.70],
    [0.75, 0.90, 0.95],
    [0.12, 0.40, 0.30],
    [0.45, 0.20, 0.15],
    [0.85, 0.75, 0.55],
];

pub const BRUSHES: [u32; 5] = [1, 2, 4, 8, 16];

/// User-adjustable panel sizes (logical pixels) and the Explorer/Outline split.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PanelLayout {
    pub left: f32,
    pub right: f32,
    pub split: f32,
    pub show_left: bool,
    pub show_right: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Splitter {
    Left,
    Right,
    Explorer,
}

/// One undoable edit: the library before it, and what it was.
struct Step {
    library: Library,
    label: String,
}

#[derive(Resource)]
pub struct Editor {
    pub library: Library,
    saved: Library,
    undo: Vec<Step>,
    redo: Vec<Step>,
    pub workspace_dir: PathBuf,
    pub workspace_ready: bool,
    pub tabs: Vec<DocRef>,
    pub active: Option<DocRef>,
    pub views: HashMap<DocRef, DocView>,
    pub collapsed: HashSet<String>,
    pub filter: String,
    pub editing: Option<TextEdit>,
    pub menu: Option<OpenMenu>,
    pub modal: Option<Modal>,
    pub brush: u32,
    pub color: [f32; 3],
    /// Voxel painting mirrors across X.
    pub mirror: bool,
    /// The color picker's hue, kept while saturation or value is zero.
    pub hue: f32,
    /// Set when the user has confirmed quitting.
    pub quit: bool,
    pub anim: AnimState,
    pub equip: Equip,
    pub generator: GenSettings,
    pub placement: Placement,
    pub rule_focus: Option<(String, String)>,
    pub guides: bool,
    /// Width / height of the 3D viewport, for framing.
    pub aspect: f32,
    pub layout: PanelLayout,
    pub notice: Notice,
    /// Bumped whenever the UI must be rebuilt.
    pub revision: u64,
    /// Bumped whenever the 3D preview must be respawned.
    pub scene_revision: u64,
    /// Library snapshot from the start of a continuous gesture (drag edits).
    gesture: Option<Step>,
    /// Recently opened workspaces (not tracked for test runs).
    pub recent: Vec<PathBuf>,
    track_recent: bool,
}

impl Editor {
    pub fn new(options: &LaunchOptions) -> Self {
        let dir = options.workspace_dir().to_path_buf();
        let loaded = storage::load_workspace(&dir);
        let failed = loaded.is_err();
        let (library, notice) = match loaded {
            Ok(library) => (
                library,
                Notice {
                    text: format!("Opened workspace {}", dir.display()),
                    kind: NoticeKind::Info,
                },
            ),
            Err(e) => (
                Library::default(),
                Notice {
                    text: format!("Load failed: {e}"),
                    kind: NoticeKind::Error,
                },
            ),
        };
        let mut editor = Self {
            saved: library.clone(),
            library,
            undo: vec![],
            redo: vec![],
            // A workspace that fails to load stays closed, so nothing can
            // save an empty library over it.
            workspace_ready: !failed
                && (options.workspace.is_some() || options.editor_view.is_some() || options.scene),
            workspace_dir: dir,
            tabs: vec![],
            active: None,
            views: HashMap::new(),
            collapsed: HashSet::new(),
            filter: String::new(),
            editing: None,
            menu: None,
            modal: None,
            brush: 4,
            color: PALETTE[12],
            mirror: false,
            hue: 160.,
            quit: false,
            anim: AnimState::default(),
            equip: Equip::default(),
            generator: GenSettings {
                seed: 7,
                size: 7,
                houses: 3,
                plants: 45,
            },
            placement: Placement::default(),
            rule_focus: None,
            guides: true,
            aspect: 16. / 9.,
            layout: PanelLayout {
                left: ui::theme::metric::LEFT_PANEL,
                right: ui::theme::metric::RIGHT_PANEL,
                split: 0.58,
                show_left: true,
                show_right: true,
            },
            notice,
            revision: 1,
            scene_revision: 1,
            gesture: None,
            recent: if options.state_file.is_none() {
                recent::load()
            } else {
                vec![]
            },
            track_recent: options.state_file.is_none(),
        };
        let view = options
            .editor_view
            .as_deref()
            .and_then(DocKind::from_view)
            .or(options.scene.then_some(DocKind::Character));
        if let Some(kind) = view {
            if let Some(doc) = editor.docs(kind).first().copied() {
                editor.open(doc);
            }
            if options.mounts
                && let Some(mount) = editor.active_body().and_then(|b| b.mounts.first())
            {
                editor.view_mut().sel = Sel::Mount(mount.id);
            }
        } else if editor.workspace_ready {
            // Like reopening a project: show the first character, else anything.
            let first = [
                DocKind::Character,
                DocKind::Body,
                DocKind::Prop,
                DocKind::World,
            ]
            .into_iter()
            .find_map(|kind| editor.docs(kind).first().copied());
            if let Some(doc) = first {
                editor.open(doc);
            }
        }
        if editor.workspace_ready {
            editor.remember_workspace();
        } else if !failed {
            editor.notice = Notice::default();
        }
        if options.browser {
            editor.modal = Some(Modal::Workspace {
                dir: editor.workspace_dir.clone(),
            });
        }
        editor
    }

    // ----- Document lookup -------------------------------------------------

    pub fn docs(&self, kind: DocKind) -> Vec<DocRef> {
        let l = &self.library;
        let ids: Vec<u32> = match kind {
            DocKind::Body => l.bodies.iter().map(|v| v.id).collect(),
            DocKind::Prop => l.props.iter().map(|v| v.id).collect(),
            DocKind::Character => l.characters.iter().map(|v| v.id).collect(),
            DocKind::Tile => l.tiles.iter().map(|v| v.id).collect(),
            DocKind::World => l.worlds.iter().map(|v| v.id).collect(),
            DocKind::Rules => (!l.tiles.is_empty() || !l.socket_rules.pairs.is_empty())
                .then_some(0)
                .into_iter()
                .collect(),
        };
        ids.into_iter().map(|id| DocRef { kind, id }).collect()
    }
    pub fn exists(&self, doc: DocRef) -> bool {
        self.doc_name_in(&self.library, doc).is_some()
    }
    pub fn doc_name(&self, doc: DocRef) -> String {
        self.doc_name_in(&self.library, doc)
            .unwrap_or_else(|| "missing".into())
    }
    fn doc_name_in(&self, l: &Library, doc: DocRef) -> Option<String> {
        let id = doc.id;
        match doc.kind {
            DocKind::Body => l.bodies.iter().find(|v| v.id == id).map(|v| v.name.clone()),
            DocKind::Prop => l.props.iter().find(|v| v.id == id).map(|v| v.name.clone()),
            DocKind::Character => l
                .characters
                .iter()
                .find(|v| v.id == id)
                .map(|v| v.name.clone()),
            DocKind::Tile => l.tiles.iter().find(|v| v.id == id).map(|v| v.name.clone()),
            DocKind::World => l.worlds.iter().find(|v| v.id == id).map(|v| v.name.clone()),
            DocKind::Rules => (id == 0).then(|| "Socket rules".into()),
        }
    }
    /// Whether a document differs from its last saved state.
    pub fn doc_dirty(&self, doc: DocRef) -> bool {
        fn differs<T: PartialEq>(a: &[T], b: &[T], id: impl Fn(&T) -> u32, want: u32) -> bool {
            a.iter().find(|v| id(v) == want) != b.iter().find(|v| id(v) == want)
        }
        let (a, b, id) = (&self.library, &self.saved, doc.id);
        match doc.kind {
            DocKind::Body => differs(&a.bodies, &b.bodies, |v| v.id, id),
            DocKind::Prop => differs(&a.props, &b.props, |v| v.id, id),
            DocKind::Character => differs(&a.characters, &b.characters, |v| v.id, id),
            DocKind::Tile => differs(&a.tiles, &b.tiles, |v| v.id, id),
            DocKind::World => differs(&a.worlds, &b.worlds, |v| v.id, id),
            DocKind::Rules => a.socket_rules != b.socket_rules,
        }
    }
    pub fn dirty(&self) -> bool {
        self.library != self.saved
    }
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    /// What Undo and Redo would revert or reapply.
    pub fn undo_label(&self) -> Option<&str> {
        self.undo.last().map(|s| s.label.as_str())
    }
    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|s| s.label.as_str())
    }

    pub fn active_kind(&self) -> Option<DocKind> {
        self.active.map(|d| d.kind)
    }
    fn active_id(&self, kind: DocKind) -> Option<u32> {
        self.active.filter(|d| d.kind == kind).map(|d| d.id)
    }
    pub fn active_body(&self) -> Option<&Body> {
        let id = self.active_id(DocKind::Body)?;
        self.library.bodies.iter().find(|b| b.id == id)
    }
    pub fn active_body_mut(&mut self) -> Option<&mut Body> {
        let id = self.active_id(DocKind::Body)?;
        self.library.bodies.iter_mut().find(|b| b.id == id)
    }
    pub fn active_prop(&self) -> Option<&Prop> {
        let id = self.active_id(DocKind::Prop)?;
        self.library.props.iter().find(|b| b.id == id)
    }
    pub fn active_prop_mut(&mut self) -> Option<&mut Prop> {
        let id = self.active_id(DocKind::Prop)?;
        self.library.props.iter_mut().find(|b| b.id == id)
    }
    pub fn active_character(&self) -> Option<&Character> {
        let id = self.active_id(DocKind::Character)?;
        self.library.characters.iter().find(|b| b.id == id)
    }
    pub fn active_character_mut(&mut self) -> Option<&mut Character> {
        let id = self.active_id(DocKind::Character)?;
        self.library.characters.iter_mut().find(|b| b.id == id)
    }
    pub fn active_tile(&self) -> Option<&game::tiles::Tile> {
        let id = self.active_id(DocKind::Tile)?;
        self.library.tiles.iter().find(|b| b.id == id)
    }
    pub fn active_tile_mut(&mut self) -> Option<&mut game::tiles::Tile> {
        let id = self.active_id(DocKind::Tile)?;
        self.library.tiles.iter_mut().find(|b| b.id == id)
    }
    pub fn active_world(&self) -> Option<&game::tiles::World> {
        let id = self.active_id(DocKind::World)?;
        self.library.worlds.iter().find(|b| b.id == id)
    }
    pub fn active_world_mut(&mut self) -> Option<&mut game::tiles::World> {
        let id = self.active_id(DocKind::World)?;
        self.library.worlds.iter_mut().find(|b| b.id == id)
    }
    /// The body shown in the viewport: the body document, or a character's body.
    pub fn preview_body(&self) -> Option<&Body> {
        match self.active_kind()? {
            DocKind::Body => self.active_body(),
            DocKind::Character => {
                let body = self.active_character()?.body;
                self.library.bodies.iter().find(|b| b.id == body)
            }
            _ => None,
        }
    }

    // ----- View state ------------------------------------------------------

    fn default_view(&self, doc: DocRef) -> DocView {
        let (pitch, distance) = match doc.kind {
            DocKind::Tile => (0.6, 24.),
            DocKind::World => (0.75, 150.),
            DocKind::Prop => (0.35, 6.5),
            _ => (0.12, 5.),
        };
        DocView {
            sel: Sel::None,
            cam: Cam {
                yaw: 0.6,
                pitch,
                distance,
                pan: [0.; 3],
            },
            tool: match doc.kind {
                DocKind::Prop | DocKind::Tile => Tool::Paint,
                _ => Tool::Select,
            },
            cursor: match doc.kind {
                DocKind::Prop => [8, 0, 8],
                _ => [0; 3],
            },
            turn: 0,
            framed: false,
        }
    }
    pub fn view(&self) -> Option<&DocView> {
        self.views.get(&self.active?)
    }
    /// Mutable view of the active document. Callers ensure a document is open.
    pub fn view_mut(&mut self) -> &mut DocView {
        let doc = self.active.expect("active document");
        if !self.views.contains_key(&doc) {
            let view = self.default_view(doc);
            self.views.insert(doc, view);
        }
        self.views.get_mut(&doc).unwrap()
    }
    pub fn sel(&self) -> Sel {
        self.view().map_or(Sel::None, |v| v.sel)
    }
    pub fn tool(&self) -> Tool {
        self.view().map_or(Tool::Select, |v| v.tool)
    }
    pub fn cursor(&self) -> [i32; 3] {
        self.view().map_or([0; 3], |v| v.cursor)
    }
    pub fn cam(&self) -> Cam {
        self.view().map_or(
            Cam {
                yaw: 0.6,
                pitch: 0.2,
                distance: 5.,
                pan: [0.; 3],
            },
            |v| v.cam,
        )
    }
    pub fn cam_mut(&mut self) -> Option<&mut Cam> {
        self.active?;
        Some(&mut self.view_mut().cam)
    }

    pub fn open(&mut self, doc: DocRef) {
        if !self.exists(doc) {
            return;
        }
        if !self.tabs.contains(&doc) {
            self.tabs.push(doc);
        }
        if self.active != Some(doc) {
            self.anim = AnimState::default();
            self.equip = Equip::default();
            self.menu = None;
        }
        self.active = Some(doc);
        if !self.views.contains_key(&doc) {
            let view = self.default_view(doc);
            self.views.insert(doc, view);
            // Large worlds start with their Outline groups folded.
            if let Some(w) = self.active_world()
                && w.instances.len() > 40
            {
                let categories: HashSet<String> = w
                    .instances
                    .iter()
                    .filter_map(|i| game::tiles::tile(&self.library.tiles, i.tile).ok())
                    .map(|t| format!("outline:{}:{}", doc.id, t.category))
                    .collect();
                self.collapsed.extend(categories);
            }
        }
        if !self.views[&doc].framed {
            viewport::frame(self);
            self.view_mut().framed = true;
        }
        self.workspace_ready = true;
    }

    pub fn notify(&mut self, kind: NoticeKind, text: impl Into<String>) {
        self.notice = Notice {
            text: text.into(),
            kind,
        };
    }
    pub fn changed(&mut self) {
        self.revision += 1;
    }

    // ----- History ---------------------------------------------------------

    /// Apply an action as one undoable step. Failed or invalid edits leave the
    /// library untouched and report why.
    pub fn apply(&mut self, action: Action, options: &LaunchOptions) {
        if self.editing.is_some()
            && !matches!(
                action,
                Action::CommitEdit | Action::CancelEdit | Action::Edit(_)
            )
        {
            self.commit_edit();
        }
        // Discrete actions never join an unfinished drag's undo step.
        self.end_gesture();
        let before = self.library.clone();
        let active = self.active;
        if self.notice.kind != NoticeKind::Error || !matches!(action, Action::Select(_)) {
            self.notice = Notice::default();
        }
        let step = (!matches!(action, Action::Undo | Action::Redo | Action::Revert))
            .then(|| actions::describe(&action));
        let label = format!("{action:?}");
        let result = actions::perform(self, action, options);
        if let Err(e) = result {
            self.library = before.clone();
            self.notify(NoticeKind::Error, e);
        }
        self.finish(before, step);
        if self.active != active {
            self.scene_revision += 1;
        }
        self.changed();
        info!(
            "editor_action action={label} active={:?} revision={} notice={}",
            self.active, self.revision, self.notice.text
        );
    }

    /// Validate an edit and record it under `label` (None: not undoable).
    fn finish(&mut self, before: Library, label: Option<String>) {
        if self.library != before {
            if let Err(e) = self.library.validate() {
                let message = self.rejection(e);
                self.library = before;
                self.notify(NoticeKind::Error, message);
                return;
            }
            if let Some(label) = label {
                self.undo.push(Step {
                    library: before,
                    label,
                });
                if self.undo.len() > 100 {
                    self.undo.remove(0);
                }
                self.redo.clear();
            }
            self.scene_revision += 1;
        }
        self.sanitize();
    }

    /// Explain a validation failure, naming the world an edit would break
    /// (tile edits can invalidate placed pieces).
    fn rejection(&self, error: String) -> String {
        let l = &self.library;
        let broken = l
            .worlds
            .iter()
            .find(|w| game::tiles::validate_world(w, &l.tiles, &l.socket_rules, false).is_err());
        match broken {
            Some(w) => format!("Edit rejected: it would break world {} ({error})", w.name),
            None => format!("Edit rejected: {error}"),
        }
    }

    /// Start a continuous edit (drag): intermediate changes share one undo step.
    pub fn begin_gesture(&mut self, label: impl Into<String>) {
        if self.gesture.is_none() {
            self.gesture = Some(Step {
                library: self.library.clone(),
                label: label.into(),
            });
        }
    }
    /// Apply an intermediate gesture edit without recording history.
    pub fn gesture_edit(&mut self, edit: impl FnOnce(&mut Self) -> Result<(), String>) {
        let before = self.library.clone();
        match edit(self) {
            Ok(()) if self.library != before => {
                if let Err(e) = self.library.validate() {
                    let message = self.rejection(e);
                    self.library = before;
                    self.notify(NoticeKind::Error, message);
                } else {
                    self.scene_revision += 1;
                }
            }
            Ok(()) => {}
            Err(e) => {
                self.library = before;
                self.notify(NoticeKind::Error, e);
            }
        }
        self.changed();
    }
    pub fn end_gesture(&mut self) {
        if let Some(step) = self.gesture.take() {
            self.finish(step.library, Some(step.label));
            self.changed();
        }
    }

    fn undo(&mut self) -> Result<(), String> {
        let step = self.undo.pop().ok_or("Nothing to undo")?;
        let current = std::mem::replace(&mut self.library, step.library);
        self.notify(NoticeKind::Info, format!("Undid {}", step.label));
        self.redo.push(Step {
            library: current,
            label: step.label,
        });
        Ok(())
    }
    fn redo(&mut self) -> Result<(), String> {
        let step = self.redo.pop().ok_or("Nothing to redo")?;
        let current = std::mem::replace(&mut self.library, step.library);
        self.notify(NoticeKind::Info, format!("Redid {}", step.label));
        self.undo.push(Step {
            library: current,
            label: step.label,
        });
        Ok(())
    }
    fn save(&mut self) -> Result<(), String> {
        storage::save_workspace(&self.workspace_dir, &self.library)?;
        // Resource files of documents deleted since the last save.
        for kind in DocKind::ALL {
            if kind == DocKind::Rules {
                continue;
            }
            for id in Self::ids(&self.saved, kind) {
                if !Self::ids(&self.library, kind).contains(&id) {
                    storage::delete_resource(&self.workspace_dir, kind.extension(), id)?;
                }
            }
        }
        self.saved = self.library.clone();
        info!("library_saved path={}", self.workspace_dir.display());
        Ok(())
    }
    fn ids(l: &Library, kind: DocKind) -> Vec<u32> {
        match kind {
            DocKind::Body => l.bodies.iter().map(|v| v.id).collect(),
            DocKind::Prop => l.props.iter().map(|v| v.id).collect(),
            DocKind::Character => l.characters.iter().map(|v| v.id).collect(),
            DocKind::Tile => l.tiles.iter().map(|v| v.id).collect(),
            DocKind::World => l.worlds.iter().map(|v| v.id).collect(),
            DocKind::Rules => vec![],
        }
    }
    fn revert(&mut self) -> Result<(), String> {
        let loaded = storage::load_workspace(&self.workspace_dir)?;
        let current = std::mem::replace(&mut self.library, loaded);
        self.undo.push(Step {
            library: current,
            label: "Revert".into(),
        });
        self.redo.clear();
        self.saved = self.library.clone();
        Ok(())
    }
    fn open_workspace(&mut self, path: PathBuf) -> Result<(), String> {
        std::fs::create_dir_all(&path).map_err(|e| format!("Open {}: {e}", path.display()))?;
        self.library = storage::load_workspace(&path)?;
        self.saved = self.library.clone();
        self.undo.clear();
        self.redo.clear();
        self.workspace_dir = path;
        self.workspace_ready = true;
        self.remember_workspace();
        self.tabs.clear();
        self.views.clear();
        self.active = None;
        self.modal = None;
        self.notify(
            NoticeKind::Success,
            format!("Opened workspace {}", self.workspace_dir.display()),
        );
        Ok(())
    }

    fn remember_workspace(&mut self) {
        if self.track_recent {
            recent::remember(&mut self.recent, &self.workspace_dir);
        }
    }

    /// Drop tabs and selections that refer to things that no longer exist.
    fn sanitize(&mut self) {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .copied()
            .filter(|d| self.exists(*d))
            .collect();
        self.tabs = tabs;
        if let Some(doc) = self.active
            && !self.exists(doc)
        {
            self.active = self.tabs.last().copied();
            self.anim = AnimState::default();
        }
        // A removed clip falls back to the last remaining one.
        let clips = self.active_character().map_or(0, |c| c.animations.len());
        if self.anim.clip.is_some_and(|c| c >= clips) {
            self.anim.clip = clips.checked_sub(1);
            if let Some(doc) = self.active
                && let Some(view) = self.views.get_mut(&doc)
                && matches!(view.sel, Sel::Clip(_))
            {
                view.sel = self.anim.clip.map_or(Sel::None, Sel::Clip);
            }
        }
        let docs: Vec<_> = self.views.keys().copied().collect();
        for doc in docs {
            if !self.exists(doc) {
                self.views.remove(&doc);
                continue;
            }
            let sel = self.views[&doc].sel;
            if !self.sel_exists(doc, sel) {
                self.views.get_mut(&doc).unwrap().sel = Sel::None;
            }
        }
        if self.anim.blend_target.is_some_and(|c| c >= clips) {
            self.anim.blend_target = None;
        }
        if let Some(index) = self.anim.clip {
            let duration = self.active_character().unwrap().animations[index].duration;
            self.anim.time = self.anim.time.clamp(0., duration);
        }
    }

    /// A short name for something selectable in the active document.
    pub fn sel_label(&self, sel: Sel) -> Option<String> {
        let tiles = &self.library.tiles;
        Some(match sel {
            Sel::None | Sel::Attachment(_) | Sel::Clip(_) => return None,
            Sel::Bone(id) => self
                .preview_body()?
                .bones
                .iter()
                .find(|b| b.id == id)?
                .name
                .clone(),
            Sel::Mount(id) => {
                let name = &self
                    .preview_body()?
                    .mounts
                    .iter()
                    .find(|m| m.id == id)?
                    .name;
                format!("{name} (mount)")
            }
            Sel::Anchor(id) => {
                let name = &self
                    .active_prop()?
                    .anchors
                    .iter()
                    .find(|a| a.id == id)?
                    .name;
                format!("{name} (anchor)")
            }
            Sel::Socket(i) => {
                let s = self.active_tile()?.sockets.get(i)?;
                format!("{} · {}", s.name, s.kind)
            }
            Sel::Collider(i) => format!("Collision box {}", i + 1),
            Sel::Piece(id) => {
                let i = self.active_world()?.instances.iter().find(|i| i.id == id)?;
                format!("{} #{id}", game::tiles::tile(tiles, i.tile).ok()?.name)
            }
            Sel::PieceSocket(id, n) => {
                let i = self.active_world()?.instances.iter().find(|i| i.id == id)?;
                let s = game::tiles::tile(tiles, i.tile).ok()?.sockets.get(n)?;
                format!("{} · {} — click to attach a tile", s.name, s.kind)
            }
        })
    }

    fn sel_exists(&self, doc: DocRef, sel: Sel) -> bool {
        let l = &self.library;
        match (doc.kind, sel) {
            (_, Sel::None) => true,
            (DocKind::Body, Sel::Bone(id) | Sel::Mount(id)) => {
                l.bodies.iter().find(|b| b.id == doc.id).is_some_and(|b| {
                    b.bones.iter().any(|v| v.id == id) || b.mounts.iter().any(|v| v.id == id)
                })
            }
            (DocKind::Prop, Sel::Anchor(id)) => l
                .props
                .iter()
                .find(|p| p.id == doc.id)
                .is_some_and(|p| p.anchors.iter().any(|a| a.id == id)),
            (DocKind::Character, sel) => {
                let Some(c) = l.characters.iter().find(|c| c.id == doc.id) else {
                    return false;
                };
                match sel {
                    Sel::Attachment(id) => c.attachments.iter().any(|a| a.id == id),
                    Sel::Clip(i) => i < c.animations.len(),
                    Sel::Bone(id) => l
                        .bodies
                        .iter()
                        .find(|b| b.id == c.body)
                        .is_some_and(|b| b.bones.iter().any(|v| v.id == id)),
                    _ => false,
                }
            }
            (DocKind::Tile, Sel::Socket(i)) => l
                .tiles
                .iter()
                .find(|t| t.id == doc.id)
                .is_some_and(|t| i < t.sockets.len()),
            (DocKind::Tile, Sel::Collider(i)) => l
                .tiles
                .iter()
                .find(|t| t.id == doc.id)
                .is_some_and(|t| i < t.colliders.len()),
            (DocKind::World, Sel::Piece(id) | Sel::PieceSocket(id, _)) => l
                .worlds
                .iter()
                .find(|w| w.id == doc.id)
                .is_some_and(|w| w.instances.iter().any(|i| i.id == id)),
            _ => false,
        }
    }

    // ----- Text editing ----------------------------------------------------

    pub fn begin_edit(&mut self, target: TextTarget) {
        let buffer = fields::text_value(self, &target);
        self.editing = Some(TextEdit {
            target,
            buffer,
            fresh: true,
            caret: usize::MAX,
        });
        self.changed();
    }
    pub fn commit_edit(&mut self) {
        let Some(edit) = self.editing.take() else {
            return;
        };
        if edit.target == TextTarget::Path {
            match self.open_workspace(PathBuf::from(edit.buffer.trim())) {
                Ok(()) => self.open_first(),
                Err(e) => self.notify(NoticeKind::Error, e),
            }
            self.scene_revision += 1;
            self.changed();
            return;
        }
        let before = self.library.clone();
        if let Err(e) = fields::commit_text(self, &edit.target, edit.buffer.trim()) {
            self.library = before.clone();
            self.notify(NoticeKind::Error, e);
        }
        let label = match &edit.target {
            TextTarget::Value(field) => format!("Edit {}", field.name()),
            TextTarget::NewRuleType => "Add socket type".into(),
            _ => "Rename".into(),
        };
        self.finish(before, Some(label));
        self.changed();
    }
}
