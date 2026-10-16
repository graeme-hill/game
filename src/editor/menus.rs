//! Popup menus: dropdown choices and context menus. Items are computed from
//! the current editor state each time the menu is drawn.
use super::*;
use game::tiles;

#[derive(Clone, Debug, PartialEq)]
pub enum Menu {
    New,
    Resource(DocRef),
    File,
    Body,
    EquipProp,
    EquipAnchor,
    EquipMount,
    BlendTarget,
    SocketKind,
    PlaceTile,
    Connect,
    View,
    /// Actions for the selected Outline item.
    Selection,
    /// New parent for the selected bone.
    Parent,
    Tab(DocRef),
}

pub struct MenuItem {
    pub label: String,
    pub action: Action,
    pub icon: Option<DocKind>,
    pub hint: String,
    pub checked: bool,
    pub separator: bool,
}

impl MenuItem {
    fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            action,
            icon: None,
            hint: String::new(),
            checked: false,
            separator: false,
        }
    }
    fn icon(mut self, kind: DocKind) -> Self {
        self.icon = Some(kind);
        self
    }
    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = hint.into();
        self
    }
    fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    fn separated(mut self) -> Self {
        self.separator = true;
        self
    }
}

pub fn items(e: &Editor, menu: &Menu) -> Vec<MenuItem> {
    let l = &e.library;
    match menu {
        Menu::New => {
            let mut items: Vec<_> = DocKind::ALL
                .into_iter()
                .filter(|k| *k != DocKind::Rules)
                .map(|k| MenuItem::new(k.label(), Action::New(k)).icon(k))
                .collect();
            if e.docs(DocKind::Rules).is_empty() {
                items.push(
                    MenuItem::new("Socket rules", Action::New(DocKind::Rules))
                        .icon(DocKind::Rules)
                        .separated(),
                );
            }
            if l.tiles.is_empty() {
                items.push(
                    MenuItem::new("Starter tile kit + world", Action::InstallStarterKit)
                        .icon(DocKind::Tile)
                        .separated(),
                );
            }
            items
        }
        Menu::Resource(doc) => {
            let mut items = vec![MenuItem::new("Open", Action::Open(*doc))];
            if doc.kind != DocKind::Rules {
                items.push(MenuItem::new("Rename", Action::Rename(*doc)).hint("F2"));
                items.push(MenuItem::new("Duplicate", Action::Duplicate(*doc)).hint("Ctrl+D"));
                items.push(
                    MenuItem::new("Delete", Action::Delete(*doc))
                        .hint("Del")
                        .separated(),
                );
            }
            items
        }
        Menu::File => {
            let mut items =
                vec![MenuItem::new("Open workspace…", Action::ShowWorkspaceBrowser).hint("Ctrl+O")];
            for (n, path) in e
                .recent
                .iter()
                .filter(|p| **p != e.workspace_dir)
                .take(4)
                .enumerate()
            {
                let name = path.file_name().map_or(path.display().to_string(), |n| {
                    n.to_string_lossy().into_owned()
                });
                let item =
                    MenuItem::new(format!("Open {name}"), Action::OpenWorkspace(path.clone()))
                        .hint("recent");
                items.push(if n == 0 { item.separated() } else { item });
            }
            items.extend([
                MenuItem::new("Save all", Action::Save)
                    .hint("Ctrl+S")
                    .separated(),
                MenuItem::new("Revert to saved", Action::Revert),
                MenuItem::new("Play", Action::Play).hint("F5").separated(),
                MenuItem::new("Quit", Action::Quit)
                    .hint("Ctrl+Q")
                    .separated(),
            ]);
            items
        }
        Menu::Body => {
            let current = e.active_character().map(|c| c.body);
            l.bodies
                .iter()
                .map(|b| {
                    MenuItem::new(&b.name, Action::SetBody(b.id))
                        .icon(DocKind::Body)
                        .hint(format!("{} bones", b.bones.len()))
                        .checked(current == Some(b.id))
                })
                .collect()
        }
        Menu::EquipProp => l
            .props
            .iter()
            .map(|p| {
                MenuItem::new(&p.name, Action::EquipProp(p.id))
                    .icon(DocKind::Prop)
                    .hint(format!("{} anchors", p.anchors.len()))
                    .checked(e.equip.prop == Some(p.id))
            })
            .collect(),
        Menu::EquipAnchor => equip_prop(e)
            .map(|p| {
                p.anchors
                    .iter()
                    .map(|a| {
                        MenuItem::new(&a.name, Action::EquipAnchor(a.id))
                            .checked(equip_anchor(e) == Some(a.id))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Menu::EquipMount => e
            .preview_body()
            .map(|b| {
                b.mounts
                    .iter()
                    .map(|m| {
                        let bone = b
                            .bones
                            .iter()
                            .find(|bone| bone.id == m.bone)
                            .map_or("", |bone| bone.name.as_str());
                        MenuItem::new(&m.name, Action::EquipMount(m.id))
                            .hint(bone)
                            .checked(equip_mount(e) == Some(m.id))
                    })
                    .collect()
            })
            .unwrap_or_default(),
        Menu::BlendTarget => {
            let mut items = vec![
                MenuItem::new("No blend", Action::Anim(AnimOp::BlendWith(None)))
                    .checked(e.anim.blend_target.is_none()),
            ];
            if let Some(c) = e.active_character() {
                for (i, clip) in c.animations.iter().enumerate() {
                    if Some(i) != e.anim.clip {
                        items.push(
                            MenuItem::new(&clip.name, Action::Anim(AnimOp::BlendWith(Some(i))))
                                .checked(e.anim.blend_target == Some(i)),
                        );
                    }
                }
            }
            items
        }
        Menu::SocketKind => {
            let current = match e.sel() {
                Sel::Socket(i) => e
                    .active_tile()
                    .and_then(|t| t.sockets.get(i))
                    .map(|s| s.kind.clone()),
                _ => None,
            };
            let mut items = vec![MenuItem::new(
                "New type…",
                Action::Edit(TextTarget::SocketKind),
            )];
            for (n, k) in actions::socket_kinds(l).into_iter().enumerate() {
                let item = MenuItem::new(&k, Action::SetSocketKind(k.clone()))
                    .checked(current.as_deref() == Some(&k));
                items.push(if n == 0 { item.separated() } else { item });
            }
            items
        }
        Menu::PlaceTile => l
            .tiles
            .iter()
            .map(|t| {
                MenuItem::new(&t.name, Action::SetPlaceTile(t.id))
                    .icon(DocKind::Tile)
                    .hint(&t.category)
                    .checked(e.placement.tile == Some(t.id))
            })
            .collect(),
        Menu::Connect => world::options(e)
            .into_iter()
            .filter_map(|(tile, turn)| {
                let t = tiles::tile(&l.tiles, tile).ok()?;
                let degrees = turn % t.rotations.len() * 360 / t.rotations.len().max(1);
                Some(
                    MenuItem::new(&t.name, Action::PlaceOption { tile, turn })
                        .icon(DocKind::Tile)
                        .hint(if t.rotations.len() > 1 {
                            format!("{degrees}°")
                        } else {
                            String::new()
                        }),
                )
            })
            .collect(),
        Menu::Tab(doc) => {
            let mut items = vec![
                MenuItem::new("Close", Action::CloseTab(*doc)).hint("Ctrl+W"),
                MenuItem::new("Close others", Action::CloseOtherTabs(*doc)),
                MenuItem::new("Close all", Action::CloseAllTabs),
            ];
            if doc.kind != DocKind::Rules {
                items.push(
                    MenuItem::new("Rename", Action::Rename(*doc))
                        .hint("F2")
                        .separated(),
                );
                items.push(MenuItem::new("Duplicate", Action::Duplicate(*doc)));
            }
            items
        }
        Menu::Parent => {
            let (Some(body), Sel::Bone(bone)) = (e.active_body(), e.sel()) else {
                return vec![];
            };
            let current = body
                .bones
                .iter()
                .find(|b| b.id == bone)
                .and_then(|b| b.parent);
            // Exclude the bone and its descendants: they would form a cycle.
            let in_branch = |mut id: u32| loop {
                if id == bone {
                    return true;
                }
                match body
                    .bones
                    .iter()
                    .find(|b| b.id == id)
                    .and_then(|b| b.parent)
                {
                    Some(p) => id = p,
                    None => return false,
                }
            };
            let mut items = vec![
                MenuItem::new("None (root bone)", Action::SetParent(None))
                    .checked(current.is_none()),
            ];
            for b in body.bones.iter().filter(|b| !in_branch(b.id)) {
                items.push(
                    MenuItem::new(&b.name, Action::SetParent(Some(b.id)))
                        .checked(current == Some(b.id)),
                );
            }
            items
        }
        Menu::Selection => {
            let delete = |label: &str| {
                MenuItem::new(label, Action::DeleteSelection)
                    .hint("Del")
                    .separated()
            };
            match (e.active_kind(), e.sel()) {
                (Some(DocKind::Body), Sel::Bone(_)) => vec![
                    MenuItem::new("Add child bone", Action::AddBone { child: true }),
                    MenuItem::new("Add mount", Action::AddMount),
                    MenuItem::new("Duplicate branch", Action::DuplicateSelection).hint("Ctrl+D"),
                    MenuItem::new("Mirror branch", Action::MirrorBranch),
                    MenuItem::new("Rename", Action::Edit(TextTarget::BoneName)).hint("F2"),
                    delete("Delete branch"),
                ],
                (_, Sel::Mount(_)) => vec![
                    MenuItem::new("Rename", Action::Edit(TextTarget::MountName)).hint("F2"),
                    MenuItem::new("Duplicate", Action::DuplicateSelection).hint("Ctrl+D"),
                    delete("Delete mount"),
                ],
                (_, Sel::Anchor(_)) => vec![
                    MenuItem::new("Rename", Action::Edit(TextTarget::AnchorName)).hint("F2"),
                    MenuItem::new("Duplicate", Action::DuplicateSelection).hint("Ctrl+D"),
                    delete("Delete anchor"),
                ],
                (_, Sel::Attachment(_)) => vec![delete("Remove from character")],
                (_, Sel::Clip(_)) => vec![
                    MenuItem::new("Play / pause", Action::Anim(AnimOp::Play)).hint("Space"),
                    MenuItem::new("Rename", Action::Edit(TextTarget::ClipName)).hint("F2"),
                    MenuItem::new("Duplicate", Action::Anim(AnimOp::Duplicate)),
                    delete("Delete animation"),
                ],
                (_, Sel::Socket(_)) => vec![
                    MenuItem::new("Rename", Action::Edit(TextTarget::SocketName)).hint("F2"),
                    MenuItem::new("Duplicate", Action::DuplicateSelection).hint("Ctrl+D"),
                    MenuItem::new("Turn 90°", Action::TurnSocket),
                    MenuItem::new("Toggle required", Action::ToggleRequired),
                    delete("Delete socket"),
                ],
                (_, Sel::Collider(_)) => vec![
                    MenuItem::new("Duplicate", Action::DuplicateSelection).hint("Ctrl+D"),
                    delete("Delete box"),
                ],
                (_, Sel::Piece(id) | Sel::PieceSocket(id, _)) => {
                    let tile = e
                        .active_world()
                        .and_then(|w| w.instances.iter().find(|i| i.id == id))
                        .map(|i| i.tile);
                    let mut items = vec![];
                    if let Some(tile) = tile {
                        items.push(
                            MenuItem::new(
                                "Open tile",
                                Action::Open(DocRef {
                                    kind: DocKind::Tile,
                                    id: tile,
                                }),
                            )
                            .icon(DocKind::Tile),
                        );
                    }
                    items.push(MenuItem::new("Complete piece", Action::CompletePiece));
                    items.push(
                        MenuItem::new("Remove piece", Action::RemovePiece)
                            .hint("Del")
                            .separated(),
                    );
                    items
                }
                _ => vec![],
            }
        }
        Menu::View => vec![
            MenuItem::new("Frame all", Action::Frame).hint("F"),
            MenuItem::new("Front", Action::View(actions::ViewPreset::Front))
                .hint("1")
                .separated(),
            MenuItem::new("Side", Action::View(actions::ViewPreset::Side)).hint("3"),
            MenuItem::new("Top", Action::View(actions::ViewPreset::Top)).hint("7"),
            MenuItem::new(
                "Perspective",
                Action::View(actions::ViewPreset::Perspective),
            )
            .hint("0"),
            MenuItem::new("Show guides", Action::ToggleGuides)
                .hint("G")
                .checked(e.guides)
                .separated(),
            MenuItem::new("Explorer panel", Action::TogglePanel(Splitter::Left))
                .checked(e.layout.show_left),
            MenuItem::new("Inspector panel", Action::TogglePanel(Splitter::Right))
                .checked(e.layout.show_right),
            MenuItem::new("Focus viewport", Action::FocusViewport).hint("\\"),
        ],
    }
}

/// The prop chosen for equipping (defaults to the first).
pub fn equip_prop(e: &Editor) -> Option<&Prop> {
    let l = &e.library;
    e.equip
        .prop
        .and_then(|id| l.props.iter().find(|p| p.id == id))
        .or(l.props.first())
}
pub fn equip_anchor(e: &Editor) -> Option<u32> {
    let p = equip_prop(e)?;
    e.equip
        .anchor
        .filter(|a| p.anchors.iter().any(|v| v.id == *a))
        .or(p.anchors.first().map(|a| a.id))
}
pub fn equip_mount(e: &Editor) -> Option<u32> {
    let b = e.preview_body()?;
    e.equip
        .mount
        .filter(|m| b.mounts.iter().any(|v| v.id == *m))
        .or(b.mounts.first().map(|m| m.id))
}
