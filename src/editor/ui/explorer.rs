//! Left panel: the workspace Explorer (resources grouped by kind, tiles by
//! category) and the Outline of the active document's parts.
use super::theme::{kind_glyph, metric, size};
use super::widgets::*;
use super::*;
use game::tiles;

pub fn left_panel(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    p.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            width: px(cx.left_width()),
            flex_shrink: 0.,
            flex_direction: FlexDirection::Column,
            border: UiRect::right(px(1)),
            ..default()
        },
        BackgroundColor(t.panel),
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        let split = cx.e.layout.split;
        p.spawn(Node {
            flex_grow: split,
            flex_basis: px(0),
            min_height: px(90),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|p| explorer(p, cx));
        if cx.e.active.is_some() {
            p.spawn(Node {
                flex_grow: 1. - split,
                flex_basis: px(0),
                min_height: px(90),
                flex_direction: FlexDirection::Column,
                border: UiRect::top(px(1)),
                position_type: PositionType::Relative,
                ..default()
            })
            .insert(BorderColor::all(t.divider))
            .with_children(|p| {
                splitter(p, cx, Splitter::Explorer);
                outline(p, cx);
            });
        }
        splitter(p, cx, Splitter::Left);
    });
}

fn panel_header(
    p: Kids,
    cx: &Ctx,
    title: &str,
    subtitle: Option<String>,
    actions: impl FnOnce(Kids),
) {
    let t = &cx.t;
    p.spawn((
        Node {
            height: px(34),
            flex_shrink: 0.,
            padding: UiRect {
                left: px(12),
                right: px(6),
                ..default()
            },
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..default()
        },
        BackgroundColor(t.panel_header),
    ))
    .with_children(|p| {
        p.spawn(text(cx, title, size::XS, t.text_dim, Weight::Semibold));
        if let Some(sub) = subtitle {
            p.spawn(text(
                cx,
                ellipsize(&sub, 22),
                size::SM,
                t.text_muted,
                Weight::Regular,
            ));
        }
        spacer(p);
        actions(p);
    });
}

fn explorer(p: Kids, cx: &Ctx) {
    let e = cx.e;
    panel_header(p, cx, "EXPLORER", None, |p| {
        button(
            p,
            cx,
            Btn::new("", Action::OpenMenu(Menu::New))
                .icon("+")
                .ghost()
                .height(24.)
                .tip("New resource (Ctrl+N)"),
        );
    });
    p.spawn(Node {
        padding: UiRect::axes(px(8), px(6)),
        flex_shrink: 0.,
        ..default()
    })
    .with_children(|p| {
        p.spawn(Node {
            column_gap: px(4),
            align_items: AlignItems::Center,
            flex_grow: 1.,
            ..default()
        })
        .with_children(|p| {
            text_input(
                p,
                cx,
                TextTarget::Filter,
                &e.filter,
                "Filter resources…",
                true,
            );
            if !e.filter.is_empty() {
                button(
                    p,
                    cx,
                    Btn::new("", Action::ClearFilter)
                        .icon("✕")
                        .ghost()
                        .height(24.)
                        .tip("Clear the filter"),
                );
            }
        });
    });
    let filter = e.filter.to_lowercase();
    scroll(p, cx, "explorer", UiRect::bottom(px(8)), 0., |p| {
        let mut any = false;
        for kind in DocKind::ALL {
            let docs: Vec<DocRef> = e
                .docs(kind)
                .into_iter()
                .filter(|d| filter.is_empty() || e.doc_name(*d).to_lowercase().contains(&filter))
                .collect();
            if kind == DocKind::Rules {
                if let Some(doc) = docs.first() {
                    any = true;
                    resource_row(p, cx, *doc, 0);
                }
                continue;
            }
            if !filter.is_empty() && docs.is_empty() {
                continue;
            }
            any = true;
            let key = format!("kind:{}", kind.plural());
            let open = !e.collapsed.contains(&key) || !filter.is_empty();
            folder_row(
                p,
                cx,
                &key,
                kind.plural(),
                Some(kind),
                docs.len(),
                open,
                0,
                Some(Action::New(kind)),
            );
            if !open {
                continue;
            }
            if docs.is_empty() {
                p.spawn((
                    Node {
                        height: px(metric::ROW),
                        padding: UiRect::left(px(34)),
                        align_items: AlignItems::Center,
                        flex_shrink: 0.,
                        ..default()
                    },
                    children![text(
                        cx,
                        "No resources yet",
                        size::SM,
                        cx.t.text_muted,
                        Weight::Regular
                    )],
                ));
                continue;
            }
            if kind == DocKind::Tile {
                // Tiles group into their categories.
                let mut categories: Vec<String> =
                    e.library.tiles.iter().map(|t| t.category.clone()).collect();
                categories.sort();
                categories.dedup();
                for category in categories {
                    let members: Vec<_> = docs
                        .iter()
                        .copied()
                        .filter(|d| {
                            tiles::tile(&e.library.tiles, d.id)
                                .is_ok_and(|t| t.category == category)
                        })
                        .collect();
                    if members.is_empty() {
                        continue;
                    }
                    let key = format!("tiles:{category}");
                    let open = !e.collapsed.contains(&key) || !filter.is_empty();
                    folder_row(p, cx, &key, &category, None, members.len(), open, 1, None);
                    if open {
                        for doc in members {
                            resource_row(p, cx, doc, 2);
                        }
                    }
                }
            } else {
                for doc in docs {
                    resource_row(p, cx, doc, 1);
                }
            }
        }
        if !any {
            p.spawn((
                Node {
                    padding: UiRect::all(px(16)),
                    flex_shrink: 0.,
                    ..default()
                },
                children![wrapped(
                    cx,
                    if filter.is_empty() {
                        "This workspace is empty. Use + to create a body, prop or tile."
                    } else {
                        "No resources match the filter."
                    },
                    size::SM,
                    cx.t.text_muted
                )],
            ));
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn folder_row(
    p: Kids,
    cx: &Ctx,
    key: &str,
    name: &str,
    kind: Option<DocKind>,
    count: usize,
    open: bool,
    depth: usize,
    add: Option<Action>,
) {
    let t = &cx.t;
    p.spawn((
        Button,
        control(
            Action::ToggleFolder(key.to_string()),
            name.to_string(),
            ControlKind::Button,
        ),
        Node {
            height: px(metric::ROW),
            flex_shrink: 0.,
            padding: UiRect {
                left: px(8. + depth as f32 * 14.),
                right: px(6),
                ..default()
            },
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..default()
        },
        BackgroundColor(Color::NONE),
        HoverStyle {
            idle: Color::NONE,
            hover: t.hover.with_alpha(0.6),
            press: t.press,
        },
    ))
    .with_children(|p| {
        p.spawn((
            Node {
                width: px(10),
                ..default()
            },
            children![glyph(cx, if open { "▾" } else { "▸" }, 10., t.text_muted)],
        ));
        if kind.is_some() {
            p.spawn(text(cx, name, size::MD, t.text, Weight::Semibold));
        } else {
            p.spawn(glyph(cx, "▤", 10., t.text_muted));
            p.spawn(text(cx, name, size::MD, t.text_dim, Weight::Medium));
        }
        p.spawn(text(
            cx,
            count.to_string(),
            size::XS,
            t.text_muted,
            Weight::Regular,
        ));
        spacer(p);
        if let Some(action) = add {
            let mut c = control(action, format!("New {}", name), ControlKind::Button);
            c.tooltip = format!("New {}", kind.map_or("", |k| k.label()).to_lowercase());
            p.spawn((
                Button,
                c,
                Node {
                    width: px(18),
                    height: px(18),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(Color::NONE),
                HoverStyle {
                    idle: Color::NONE,
                    hover: t.press,
                    press: t.press,
                },
                children![glyph(cx, "+", 12., t.text_muted)],
            ));
        }
    });
}

fn resource_row(p: Kids, cx: &Ctx, doc: DocRef, depth: usize) {
    let t = &cx.t;
    let e = cx.e;
    let active = e.active == Some(doc);
    let renaming = e
        .editing
        .as_ref()
        .is_some_and(|x| x.target == TextTarget::DocName(doc));
    if renaming {
        p.spawn(Node {
            height: px(metric::ROW + 2.),
            flex_shrink: 0.,
            padding: UiRect {
                left: px(8. + depth as f32 * 14.),
                right: px(8),
                ..default()
            },
            align_items: AlignItems::Center,
            column_gap: px(6),
            ..default()
        })
        .with_children(|p| {
            p.spawn(glyph(cx, kind_glyph(doc.kind), 11., t.kind(doc.kind)));
            text_input(p, cx, TextTarget::DocName(doc), &e.doc_name(doc), "", true);
        });
        return;
    }
    let (idle, fg) = if active {
        (t.accent_dim, t.text)
    } else {
        (Color::NONE, t.text_dim)
    };
    let mut c = control(Action::Open(doc), e.doc_name(doc), ControlKind::Button);
    c.context = Some(Menu::Resource(doc));
    p.spawn((
        Button,
        c,
        Node {
            height: px(metric::ROW),
            flex_shrink: 0.,
            padding: UiRect {
                left: px(10. + depth as f32 * 14.),
                right: px(8),
                ..default()
            },
            align_items: AlignItems::Center,
            column_gap: px(7),
            border: UiRect::left(px(2)),
            ..default()
        },
        BackgroundColor(idle),
        BorderColor::all(if active { t.accent } else { Color::NONE }),
        HoverStyle {
            idle,
            hover: if active {
                idle
            } else {
                t.hover.with_alpha(0.6)
            },
            press: t.press,
        },
    ))
    .insert_if(Reveal, || active)
    .with_children(|p| {
        p.spawn(glyph(cx, kind_glyph(doc.kind), 11., t.kind(doc.kind)));
        p.spawn(text(
            cx,
            ellipsize(&e.doc_name(doc), 26),
            size::MD,
            fg,
            if active {
                Weight::Medium
            } else {
                Weight::Regular
            },
        ));
        spacer(p);
        if e.doc_dirty(doc) {
            p.spawn(glyph(cx, "●", 8., t.warning));
        }
    });
}

/// An optional group title (with count) shown above an outline row.
type GroupHeader = Option<Group>;

/// An Outline group title; groups with a key can be collapsed.
struct Group {
    title: String,
    count: usize,
    key: Option<String>,
    /// Shown expanded regardless (it holds the selection).
    force_open: bool,
}

fn header(title: &str, count: usize) -> Group {
    Group {
        title: title.into(),
        count,
        key: None,
        force_open: false,
    }
}

/// One selectable outline row.
struct Item {
    depth: usize,
    glyph: &'static str,
    color: Color,
    label: String,
    detail: String,
    sel: Sel,
}

fn item_row(p: Kids, cx: &Ctx, item: Item, selected: bool) {
    let t = &cx.t;
    let (idle, fg) = if selected {
        (t.accent_dim, t.text)
    } else {
        (Color::NONE, t.text_dim)
    };
    let mut c = control(
        Action::Select(item.sel),
        item.label.clone(),
        ControlKind::Button,
    );
    if item.sel != Sel::None {
        c.context = Some(Menu::Selection);
    }
    p.spawn((
        Button,
        c,
        Node {
            height: px(metric::ROW - 2.),
            flex_shrink: 0.,
            padding: UiRect {
                left: px(12. + item.depth as f32 * 14.),
                right: px(8),
                ..default()
            },
            align_items: AlignItems::Center,
            column_gap: px(7),
            ..default()
        },
        BackgroundColor(idle),
        HoverStyle {
            idle,
            hover: if selected {
                idle
            } else {
                t.hover.with_alpha(0.6)
            },
            press: t.press,
        },
    ))
    .insert_if(Reveal, || selected && item.sel != Sel::None)
    .with_children(|p| {
        if item.depth > 0 {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(4. + item.depth as f32 * 14.),
                    top: px(0),
                    bottom: px(0),
                    width: px(1),
                    ..default()
                },
                BackgroundColor(t.border),
            ));
        }
        p.spawn(glyph(cx, item.glyph, 10., item.color));
        p.spawn(text(
            cx,
            ellipsize(&item.label, 24),
            size::MD,
            fg,
            Weight::Regular,
        ));
        spacer(p);
        if !item.detail.is_empty() {
            p.spawn(text(
                cx,
                ellipsize(&item.detail, 14),
                size::XS,
                t.text_muted,
                Weight::Regular,
            ));
        }
    });
}

fn group(p: Kids, cx: &Ctx, g: &Group, open: bool) {
    let t = &cx.t;
    let mut row = p.spawn(Node {
        height: px(24),
        padding: UiRect {
            left: px(12),
            top: px(6),
            ..default()
        },
        align_items: AlignItems::Center,
        column_gap: px(6),
        flex_shrink: 0.,
        ..default()
    });
    if let Some(key) = &g.key {
        row.insert((
            Button,
            control(
                Action::ToggleFolder(key.clone()),
                g.title.clone(),
                ControlKind::Button,
            ),
            BackgroundColor(Color::NONE),
            HoverStyle {
                idle: Color::NONE,
                hover: t.hover.with_alpha(0.5),
                press: t.press,
            },
        ));
    }
    row.with_children(|p| {
        if g.key.is_some() {
            p.spawn(glyph(cx, if open { "▾" } else { "▸" }, 9., t.text_muted));
        }
        p.spawn(text(
            cx,
            g.title.to_uppercase(),
            size::XS,
            t.text_muted,
            Weight::Semibold,
        ));
        p.spawn(text(
            cx,
            g.count.to_string(),
            size::XS,
            t.text_muted,
            Weight::Regular,
        ));
    });
}

fn bone_items(body: &Body, mounts: bool, t: &Palette) -> Vec<Item> {
    fn visit(
        body: &Body,
        parent: Option<u32>,
        depth: usize,
        mounts: bool,
        t: &Palette,
        out: &mut Vec<Item>,
    ) {
        for bone in body.bones.iter().filter(|b| b.parent == parent) {
            out.push(Item {
                depth,
                glyph: match bone.shape {
                    BodyShape::Sphere => "●",
                    BodyShape::Capsule => "▮",
                },
                color: Color::srgb(bone.color[0], bone.color[1], bone.color[2]),
                label: bone.name.clone(),
                detail: String::new(),
                sel: Sel::Bone(bone.id),
            });
            if mounts {
                for m in body.mounts.iter().filter(|m| m.bone == bone.id) {
                    out.push(Item {
                        depth: depth + 1,
                        glyph: "✦",
                        color: t.warning,
                        label: m.name.clone(),
                        detail: "mount".into(),
                        sel: Sel::Mount(m.id),
                    });
                }
            }
            visit(body, Some(bone.id), depth + 1, mounts, t, out);
        }
    }
    let mut out = vec![];
    visit(body, None, 0, mounts, t, &mut out);
    out
}

fn outline(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let Some(doc) = e.active else { return };
    let sel = e.sel();
    panel_header(p, cx, "OUTLINE", Some(e.doc_name(doc)), |p| {
        match doc.kind {
            DocKind::Body => {
                button(
                    p,
                    cx,
                    Btn::new(
                        "",
                        Action::AddBone {
                            child: matches!(sel, Sel::Bone(_)),
                        },
                    )
                    .icon("+")
                    .ghost()
                    .height(24.)
                    .tip(if matches!(sel, Sel::Bone(_)) {
                        "Add a child bone to the selection"
                    } else {
                        "Add a root bone"
                    }),
                );
            }
            DocKind::Prop => {
                button(
                    p,
                    cx,
                    Btn::new("", Action::AddAnchor)
                        .icon("+")
                        .ghost()
                        .height(24.)
                        .tip("Add an anchor at the voxel cursor"),
                );
            }
            DocKind::Character => {
                button(
                    p,
                    cx,
                    Btn::new("", Action::Anim(AnimOp::New))
                        .icon("+")
                        .ghost()
                        .height(24.)
                        .tip("New animation clip"),
                );
            }
            DocKind::Tile => {
                button(
                    p,
                    cx,
                    Btn::new("", Action::AddSocket)
                        .icon("+")
                        .ghost()
                        .height(24.)
                        .tip("Add a socket at the cursor"),
                );
            }
            _ => {}
        }
    });
    scroll(
        p,
        cx,
        &format!("outline:{doc:?}"),
        UiRect::bottom(px(8)),
        0.,
        |p| {
            let mut items: Vec<(GroupHeader, Item)> = vec![];
            let doc_item = |label: String, detail: String| Item {
                depth: 0,
                glyph: kind_glyph(doc.kind),
                color: t.kind(doc.kind),
                label,
                detail,
                sel: Sel::None,
            };
            match doc.kind {
                DocKind::Body => {
                    if let Some(body) = e.active_body() {
                        items.push((
                            None,
                            doc_item(body.name.clone(), format!("{} bones", body.bones.len())),
                        ));
                        let bones = bone_items(body, true, t);
                        let n = body.bones.len();
                        for (i, item) in bones.into_iter().enumerate() {
                            let header = (i == 0).then(|| header("Skeleton", n));
                            items.push((header, item));
                        }
                    }
                }
                DocKind::Prop => {
                    if let Some(prop) = e.active_prop() {
                        items.push((
                            None,
                            doc_item("Voxels".into(), format!("{} blocks", prop.blocks.len())),
                        ));
                        for (i, a) in prop.anchors.iter().enumerate() {
                            items.push((
                                (i == 0).then(|| header("Anchors", prop.anchors.len())),
                                Item {
                                    depth: 0,
                                    glyph: "✦",
                                    color: t.warning,
                                    label: a.name.clone(),
                                    detail: String::new(),
                                    sel: Sel::Anchor(a.id),
                                },
                            ));
                        }
                    }
                }
                DocKind::Character => {
                    if let Some(c) = e.active_character() {
                        let body = e.preview_body();
                        items.push((
                            None,
                            doc_item(
                                c.name.clone(),
                                body.map_or("no body".into(), |b| b.name.clone()),
                            ),
                        ));
                        for (i, a) in c.attachments.iter().enumerate() {
                            let prop = e.library.props.iter().find(|p| p.id == a.prop);
                            let mount =
                                body.and_then(|b| b.mounts.iter().find(|m| m.id == a.mount));
                            items.push((
                                (i == 0).then(|| header("Equipment", c.attachments.len())),
                                Item {
                                    depth: 0,
                                    glyph: kind_glyph(DocKind::Prop),
                                    color: t.kind(DocKind::Prop),
                                    label: prop.map_or("missing".into(), |p| p.name.clone()),
                                    detail: mount.map_or(String::new(), |m| m.name.clone()),
                                    sel: Sel::Attachment(a.id),
                                },
                            ));
                        }
                        for (i, clip) in c.animations.iter().enumerate() {
                            items.push((
                                (i == 0).then(|| header("Animations", c.animations.len())),
                                Item {
                                    depth: 0,
                                    glyph: "▶",
                                    color: t.kind(DocKind::Character),
                                    label: clip.name.clone(),
                                    detail: format!("{:.1}s", clip.duration),
                                    sel: Sel::Clip(i),
                                },
                            ));
                        }
                        if let Some(body) = body {
                            for (i, item) in bone_items(body, false, t).into_iter().enumerate() {
                                items.push((
                                    (i == 0).then(|| header("Skeleton", body.bones.len())),
                                    item,
                                ));
                            }
                        }
                    }
                }
                DocKind::Tile => {
                    if let Some(tile) = e.active_tile() {
                        items.push((
                            None,
                            doc_item("Voxels".into(), format!("{} volumes", tile.voxels.len())),
                        ));
                        for (i, s) in tile.sockets.iter().enumerate() {
                            items.push((
                                (i == 0).then(|| header("Sockets", tile.sockets.len())),
                                Item {
                                    depth: 0,
                                    glyph: if s.required { "◉" } else { "○" },
                                    color: if s.required {
                                        t.socket_required
                                    } else {
                                        t.socket
                                    },
                                    label: s.name.clone(),
                                    detail: s.kind.clone(),
                                    sel: Sel::Socket(i),
                                },
                            ));
                        }
                        for i in 0..tile.colliders.len() {
                            items.push((
                                (i == 0).then(|| header("Collision", tile.colliders.len())),
                                Item {
                                    depth: 0,
                                    glyph: "▣",
                                    color: t.warning,
                                    label: format!("Box {}", i + 1),
                                    detail: String::new(),
                                    sel: Sel::Collider(i),
                                },
                            ));
                        }
                    }
                }
                DocKind::World => {
                    if let Some(w) = e.active_world() {
                        items.push((
                            None,
                            doc_item(w.name.clone(), format!("{} pieces", w.instances.len())),
                        ));
                        let selected_piece = match sel {
                            Sel::Piece(id) | Sel::PieceSocket(id, _) => Some(id),
                            _ => None,
                        };
                        // Pieces group by tile category; groups collapse.
                        let mut categories: Vec<(String, Vec<&game::tiles::Instance>)> = vec![];
                        for inst in &w.instances {
                            let category = tiles::tile(&e.library.tiles, inst.tile)
                                .map_or("missing".to_string(), |t| t.category.clone());
                            match categories.iter_mut().find(|(c, _)| *c == category) {
                                Some((_, list)) => list.push(inst),
                                None => categories.push((category, vec![inst])),
                            }
                        }
                        categories.sort_by(|a, b| a.0.cmp(&b.0));
                        for (category, pieces) in categories {
                            let force_open = pieces.iter().any(|i| Some(i.id) == selected_piece);
                            let mut group = Some(Group {
                                title: category.clone(),
                                count: pieces.len(),
                                key: Some(format!("outline:{}:{category}", doc.id)),
                                force_open,
                            });
                            for inst in pieces {
                                let tile = tiles::tile(&e.library.tiles, inst.tile).ok();
                                items.push((
                                    group.take(),
                                    Item {
                                        depth: 0,
                                        glyph: kind_glyph(DocKind::Tile),
                                        color: t.kind(DocKind::Tile),
                                        label: tile.map_or("missing".into(), |t| t.name.clone()),
                                        detail: format!("#{}", inst.id),
                                        sel: Sel::Piece(inst.id),
                                    },
                                ));
                                if selected_piece == Some(inst.id)
                                    && let Some(tile) = tile
                                {
                                    for (n, s) in tile.sockets.iter().enumerate() {
                                        let connected = w.connected(tiles::Port {
                                            instance: inst.id,
                                            socket: n,
                                        });
                                        items.push((
                                            None,
                                            Item {
                                                depth: 1,
                                                glyph: if connected { "●" } else { "○" },
                                                color: if connected {
                                                    t.text_muted
                                                } else {
                                                    t.socket
                                                },
                                                label: s.name.clone(),
                                                detail: if connected {
                                                    "linked".into()
                                                } else {
                                                    "open".into()
                                                },
                                                sel: Sel::PieceSocket(inst.id, n),
                                            },
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
                DocKind::Rules => {
                    let kinds = actions::socket_kinds(&e.library);
                    for (i, k) in kinds.iter().enumerate() {
                        let partners = e
                            .library
                            .socket_rules
                            .pairs
                            .iter()
                            .filter(|p| p[0] == *k || p[1] == *k)
                            .count();
                        items.push((
                            (i == 0).then(|| header("Socket types", kinds.len())),
                            Item {
                                depth: 0,
                                glyph: "○",
                                color: t.socket,
                                label: k.clone(),
                                detail: format!("{partners} pairs"),
                                sel: Sel::None,
                            },
                        ));
                    }
                }
            }
            let mut hidden = false;
            for (header, item) in items {
                if let Some(g) = header {
                    // A collapsed group still opens for the current selection.
                    hidden =
                        !g.force_open && g.key.as_ref().is_some_and(|k| e.collapsed.contains(k));
                    group(p, cx, &g, !hidden);
                }
                if hidden {
                    continue;
                }
                let selected = item.sel == sel && !(doc.kind == DocKind::Rules);
                if doc.kind != DocKind::Rules {
                    cx.outline.borrow_mut().push(item.sel);
                }
                item_row(p, cx, item, selected);
            }
        },
    );
}
