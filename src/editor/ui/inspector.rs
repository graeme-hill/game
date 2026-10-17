//! Right panel: properties of the active document and its selection.
use super::theme::{kind_glyph, size};
use super::widgets::*;
use super::*;
use game::{tiles, voxel};

pub fn right_panel(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    p.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            width: px(cx.right_width()),
            flex_shrink: 0.,
            flex_direction: FlexDirection::Column,
            border: UiRect::left(px(1)),
            ..default()
        },
        BackgroundColor(t.panel),
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        splitter(p, cx, Splitter::Right);
        p.spawn((
            Node {
                height: px(34),
                flex_shrink: 0.,
                padding: UiRect::horizontal(px(12)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(t.panel_header),
            children![text(
                cx,
                "INSPECTOR",
                size::XS,
                t.text_dim,
                Weight::Semibold
            )],
        ));
        let Some(doc) = cx.e.active else {
            p.spawn(Node {
                flex_grow: 1.,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|p| {
                empty_state(
                    p,
                    cx,
                    "☰",
                    "No selection",
                    "Open a resource to see and edit its properties here.",
                    |_| {},
                );
            });
            return;
        };
        header(p, cx, doc);
        scroll(
            p,
            cx,
            &format!("inspector:{doc:?}"),
            UiRect::bottom(px(16)),
            0.,
            |p| match doc.kind {
                DocKind::Body => body(p, cx),
                DocKind::Prop => prop(p, cx),
                DocKind::Character => character(p, cx),
                DocKind::Tile => tile(p, cx),
                DocKind::World => world(p, cx),
                DocKind::Rules => rules(p, cx),
            },
        );
    });
}

fn header(p: Kids, cx: &Ctx, doc: DocRef) {
    let t = &cx.t;
    let e = cx.e;
    p.spawn((
        Node {
            padding: UiRect::axes(px(12), px(10)),
            column_gap: px(10),
            align_items: AlignItems::Center,
            flex_shrink: 0.,
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        p.spawn((
            Node {
                width: px(32),
                height: px(32),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(8)),
                flex_shrink: 0.,
                ..default()
            },
            BackgroundColor(t.kind(doc.kind).with_alpha(0.16)),
            children![glyph(cx, kind_glyph(doc.kind), 16., t.kind(doc.kind))],
        ));
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(3),
            flex_grow: 1.,
            flex_basis: px(0),
            min_width: px(0),
            ..default()
        })
        .with_children(|p| {
            p.spawn(text(
                cx,
                format!(
                    "{}{}",
                    doc.kind.label().to_uppercase(),
                    if e.doc_dirty(doc) {
                        "  ·  MODIFIED"
                    } else {
                        ""
                    }
                ),
                size::XS,
                t.text_muted,
                Weight::Semibold,
            ));
            if doc.kind == DocKind::Rules {
                p.spawn(text(cx, "Socket rules", size::LG, t.text, Weight::Semibold));
            } else {
                title_input(p, cx, TextTarget::DocName(doc), &e.doc_name(doc));
            }
        });
        if doc.kind != DocKind::Rules {
            button(
                p,
                cx,
                Btn::new("", Action::OpenMenu(Menu::Resource(doc)))
                    .icon("⋯")
                    .ghost()
                    .tip("More actions"),
            );
        }
    });
}

fn voxel_tool(p: Kids, cx: &Ctx, kind: DocKind) {
    let e = cx.e;
    section(p, cx, "voxel-tool", "Voxel brush", |p| {
        row(p, cx, "Tool", |p| {
            let tool = e.tool();
            let mut items = vec![];
            if kind == DocKind::Prop {
                items.push((
                    "Select".to_string(),
                    Action::SetTool(Tool::Select),
                    tool == Tool::Select,
                    "Select anchors (V)".to_string(),
                ));
            }
            items.push((
                "Paint".into(),
                Action::SetTool(Tool::Paint),
                tool == Tool::Paint,
                "Paint (B)".into(),
            ));
            items.push((
                "Erase".into(),
                Action::SetTool(Tool::Erase),
                tool == Tool::Erase,
                "Erase (E)".into(),
            ));
            segmented(p, cx, items, true);
        });
        row(p, cx, "Size", |p| {
            segmented(
                p,
                cx,
                BRUSHES
                    .iter()
                    .map(|b| {
                        (
                            b.to_string(),
                            Action::Brush(*b),
                            e.brush == *b,
                            format!("{b}³ voxels"),
                        )
                    })
                    .collect(),
                true,
            );
        });
        row(p, cx, "Color", |p| {
            for axis in 0..3 {
                number(
                    p,
                    cx,
                    Field::Rgb(axis),
                    Some((["R", "G", "B"][axis], cx.t.axis(axis))),
                    true,
                );
            }
        });
        color_picker(p, cx, e.color);
        palette(p, cx, e.color);
        vector(
            p,
            cx,
            "Cursor",
            [Field::Cursor(0), Field::Cursor(1), Field::Cursor(2)],
        );
        row(p, cx, "Symmetry", |p| {
            checkbox(p, cx, "Mirror across X", e.mirror, Action::ToggleMirror);
        });
        hstack(p, 6., |p| {
            button(
                p,
                cx,
                Btn::new("Paint at cursor", Action::PaintCursor { erase: false })
                    .icon("✎")
                    .grow(),
            );
            button(
                p,
                cx,
                Btn::new("Erase at cursor", Action::PaintCursor { erase: true })
                    .icon("⌫")
                    .grow(),
            );
        });
        hint(
            p,
            cx,
            "Click or drag in the viewport to paint on faces or the cursor layer (Y sets that layer). Alt+click picks a voxel's color.",
        );
    });
}

fn body(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let Some(b) = e.active_body() else { return };
    match e.sel() {
        Sel::Bone(id) => {
            let Some(bone) = b.bones.iter().find(|v| v.id == id) else {
                return;
            };
            section(p, cx, "bone", "Bone", |p| {
                row(p, cx, "Name", |p| {
                    text_input(p, cx, TextTarget::BoneName, &bone.name, "Name", true)
                });
                let parent = bone
                    .parent
                    .and_then(|pid| b.bones.iter().find(|v| v.id == pid))
                    .map_or("— (root)".to_string(), |v| v.name.clone());
                row(p, cx, "Parent", |p| {
                    dropdown(p, cx, Menu::Parent, &parent, None)
                });
                row(p, cx, "Shape", |p| {
                    segmented(
                        p,
                        cx,
                        vec![
                            (
                                "Capsule".into(),
                                if bone.shape == BodyShape::Capsule {
                                    Action::Select(Sel::Bone(id))
                                } else {
                                    Action::ToggleShape
                                },
                                bone.shape == BodyShape::Capsule,
                                "A rounded segment from joint to tip".into(),
                            ),
                            (
                                "Sphere".into(),
                                if bone.shape == BodyShape::Sphere {
                                    Action::Select(Sel::Bone(id))
                                } else {
                                    Action::ToggleShape
                                },
                                bone.shape == BodyShape::Sphere,
                                "A ball centered on the tip".into(),
                            ),
                        ],
                        true,
                    );
                });
                row(p, cx, "Radius", |p| {
                    number(p, cx, Field::BoneRadius, None, true)
                });
            });
            section(p, cx, "bone-transform", "Transform", |p| {
                vector(
                    p,
                    cx,
                    "Joint",
                    [
                        Field::BoneOffset(0),
                        Field::BoneOffset(1),
                        Field::BoneOffset(2),
                    ],
                );
                vector(
                    p,
                    cx,
                    "Tip",
                    [Field::BoneTip(0), Field::BoneTip(1), Field::BoneTip(2)],
                );
                hint(
                    p,
                    cx,
                    "Joint is relative to the parent's tip; tip is relative to the joint. Drag a field to scrub.",
                );
            });
            section(p, cx, "bone-color", "Color", |p| {
                row(p, cx, "RGB", |p| {
                    for axis in 0..3 {
                        number(
                            p,
                            cx,
                            Field::Rgb(axis),
                            Some((["R", "G", "B"][axis], t.axis(axis))),
                            true,
                        );
                    }
                });
                color_picker(p, cx, bone.color);
                palette(p, cx, bone.color);
            });
            section(p, cx, "bone-actions", "Hierarchy", |p| {
                hstack(p, 6., |p| {
                    button(
                        p,
                        cx,
                        Btn::new("Add child", Action::AddBone { child: true })
                            .icon("+")
                            .grow(),
                    );
                    button(
                        p,
                        cx,
                        Btn::new("Add mount", Action::AddMount).icon("✦").grow(),
                    );
                });
                button(
                    p,
                    cx,
                    Btn::new("Mirror branch", Action::MirrorBranch)
                        .icon("⇄")
                        .tip("Copy this bone and its children mirrored across X (Left ↔ Right)"),
                );
                button(
                    p,
                    cx,
                    Btn::new("Delete branch", Action::DeleteSelection)
                        .icon("✕")
                        .danger()
                        .tip("Delete this bone, its children and their mounts (Del)"),
                );
            });
        }
        Sel::Mount(id) => {
            let Some(m) = b.mounts.iter().find(|v| v.id == id) else {
                return;
            };
            section(p, cx, "mount", "Mount", |p| {
                row(p, cx, "Name", |p| {
                    text_input(p, cx, TextTarget::MountName, &m.name, "Name", true)
                });
                let bone = b
                    .bones
                    .iter()
                    .find(|v| v.id == m.bone)
                    .map_or("missing", |v| v.name.as_str());
                readout(p, cx, "Bone", bone);
                vector(
                    p,
                    cx,
                    "Offset",
                    [
                        Field::MountOffset(0),
                        Field::MountOffset(1),
                        Field::MountOffset(2),
                    ],
                );
                vector(
                    p,
                    cx,
                    "Rotation",
                    [
                        Field::MountRotation(0),
                        Field::MountRotation(1),
                        Field::MountRotation(2),
                    ],
                );
                hint(
                    p,
                    cx,
                    "Props attach their anchor here. The offset is relative to the bone's tip.",
                );
                button(
                    p,
                    cx,
                    Btn::new("Delete mount", Action::DeleteSelection)
                        .icon("✕")
                        .danger(),
                );
            });
        }
        _ => {
            section(p, cx, "body", "Body", |p| {
                readout(p, cx, "Bones", format!("{} / {}", b.bones.len(), MAX_BONES));
                readout(p, cx, "Mounts", b.mounts.len().to_string());
                let users: Vec<_> = e
                    .library
                    .characters
                    .iter()
                    .filter(|c| c.body == b.id)
                    .map(|c| c.name.clone())
                    .collect();
                readout(
                    p,
                    cx,
                    "Used by",
                    if users.is_empty() {
                        "—".to_string()
                    } else {
                        users.join(", ")
                    },
                );
            });
            section(p, cx, "body-build", "Build", |p| {
                button(
                    p,
                    cx,
                    Btn::new("Add root bone", Action::AddBone { child: false }).icon("+"),
                );
                if b.bones.is_empty() {
                    button(
                        p,
                        cx,
                        Btn::new("Humanoid template", Action::DefaultSkeleton)
                            .icon("★")
                            .primary(),
                    );
                }
                hint(
                    p,
                    cx,
                    "Select a bone in the Outline or viewport to shape it. Bodies are smooth SDF goo built from capsules and spheres.",
                );
            });
        }
    }
}

fn prop(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let Some(prop) = e.active_prop() else { return };
    if let Sel::Anchor(id) = e.sel()
        && let Some(a) = prop.anchors.iter().find(|v| v.id == id)
    {
        section(p, cx, "anchor", "Anchor", |p| {
            row(p, cx, "Name", |p| {
                text_input(p, cx, TextTarget::AnchorName, &a.name, "Name", true)
            });
            vector(
                p,
                cx,
                "Position",
                [
                    Field::AnchorPosition(0),
                    Field::AnchorPosition(1),
                    Field::AnchorPosition(2),
                ],
            );
            vector(
                p,
                cx,
                "Rotation",
                [
                    Field::AnchorRotation(0),
                    Field::AnchorRotation(1),
                    Field::AnchorRotation(2),
                ],
            );
            hint(
                p,
                cx,
                "When equipped, this point and orientation align with a body mount.",
            );
            button(
                p,
                cx,
                Btn::new("Delete anchor", Action::DeleteSelection)
                    .icon("✕")
                    .danger(),
            );
        });
    }
    voxel_tool(p, cx, DocKind::Prop);
    section(p, cx, "prop", "Prop", |p| {
        let stats = voxel::mesh_stats(prop);
        readout(p, cx, "Blocks", stats.block_count.to_string());
        readout(p, cx, "Quads", stats.quad_count.to_string());
        readout(p, cx, "Voxels", stats.occupied_cells.to_string());
        readout(p, cx, "Anchors", prop.anchors.len().to_string());
        let users: Vec<_> = e
            .library
            .characters
            .iter()
            .filter(|c| c.attachments.iter().any(|a| a.prop == prop.id))
            .map(|c| c.name.clone())
            .collect();
        readout(
            p,
            cx,
            "Used by",
            if users.is_empty() {
                "—".to_string()
            } else {
                users.join(", ")
            },
        );
        button(
            p,
            cx,
            Btn::new("Add anchor at cursor", Action::AddAnchor).icon("✦"),
        );
    });
}

fn character(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let Some(c) = e.active_character() else {
        return;
    };
    let body = e.preview_body();
    match e.sel() {
        Sel::Attachment(id) => {
            let Some(a) = c.attachments.iter().find(|v| v.id == id) else {
                return;
            };
            section(p, cx, "attachment", "Equipped prop", |p| {
                let prop = e.library.props.iter().find(|v| v.id == a.prop);
                readout(
                    p,
                    cx,
                    "Prop",
                    prop.map_or("missing".into(), |v| v.name.clone()),
                );
                readout(
                    p,
                    cx,
                    "Anchor",
                    prop.and_then(|v| v.anchors.iter().find(|x| x.id == a.anchor))
                        .map_or("missing".into(), |v| v.name.clone()),
                );
                readout(
                    p,
                    cx,
                    "Mount",
                    body.and_then(|b| b.mounts.iter().find(|m| m.id == a.mount))
                        .map_or("missing".into(), |m| m.name.clone()),
                );
                vector(
                    p,
                    cx,
                    "Offset",
                    [
                        Field::AttachOffset(0),
                        Field::AttachOffset(1),
                        Field::AttachOffset(2),
                    ],
                );
                vector(
                    p,
                    cx,
                    "Rotation",
                    [
                        Field::AttachRotation(0),
                        Field::AttachRotation(1),
                        Field::AttachRotation(2),
                    ],
                );
                row(p, cx, "Scale", |p| {
                    number(p, cx, Field::AttachScale, None, true)
                });
                button(
                    p,
                    cx,
                    Btn::new(
                        "Open prop",
                        Action::Open(DocRef {
                            kind: DocKind::Prop,
                            id: a.prop,
                        }),
                    )
                    .icon("↗")
                    .ghost(),
                );
                button(
                    p,
                    cx,
                    Btn::new("Remove from character", Action::DeleteSelection)
                        .icon("✕")
                        .danger(),
                );
            });
        }
        Sel::Clip(_) => {
            if let Some(clip) = e.animation_clip() {
                section(p, cx, "clip", "Animation", |p| {
                    row(p, cx, "Name", |p| {
                        text_input(p, cx, TextTarget::ClipName, &clip.name, "Name", true)
                    });
                    row(p, cx, "Length", |p| {
                        number(p, cx, Field::ClipDuration, None, true)
                    });
                    row(p, cx, "Playback", |p| {
                        checkbox(p, cx, "Loop", clip.looping, Action::Anim(AnimOp::Loop));
                    });
                    readout(p, cx, "Tracks", clip.tracks.len().to_string());
                    if let Some(stride) = clip.stride_distance {
                        readout(p, cx, "Stride", format!("{stride:.2} units"));
                    }
                    hstack(p, 6., |p| {
                        button(
                            p,
                            cx,
                            Btn::new(
                                if e.anim.playing { "Pause" } else { "Play" },
                                Action::Anim(AnimOp::Play),
                            )
                            .icon(if e.anim.playing { "❚❚" } else { "▶" })
                            .primary()
                            .grow(),
                        );
                        button(
                            p,
                            cx,
                            Btn::new("Duplicate", Action::Anim(AnimOp::Duplicate)).grow(),
                        );
                    });
                    button(
                        p,
                        cx,
                        Btn::new("Delete animation", Action::DeleteSelection)
                            .icon("✕")
                            .danger(),
                    );
                    hint(
                        p,
                        cx,
                        "Select a bone in the Outline or viewport, then pose it at the playhead.",
                    );
                });
            }
        }
        Sel::Bone(id) => {
            let bone = body.and_then(|b| b.bones.iter().find(|v| v.id == id));
            section(p, cx, "pose", "Pose", |p| {
                readout(
                    p,
                    cx,
                    "Bone",
                    bone.map_or("missing".into(), |b| b.name.clone()),
                );
                if e.anim.clip.is_none() {
                    hint(
                        p,
                        cx,
                        "Open an animation from the Outline to key this bone.",
                    );
                    return;
                }
                row(p, cx, "Time", |p| number(p, cx, Field::Time, None, true));
                vector(
                    p,
                    cx,
                    "Rotation",
                    [
                        Field::KeyRotation(0),
                        Field::KeyRotation(1),
                        Field::KeyRotation(2),
                    ],
                );
                vector(
                    p,
                    cx,
                    "Offset",
                    [
                        Field::KeyOffset(0),
                        Field::KeyOffset(1),
                        Field::KeyOffset(2),
                    ],
                );
                hstack(p, 6., |p| {
                    button(
                        p,
                        cx,
                        Btn::new("Set key", Action::Anim(AnimOp::Key))
                            .icon("◆")
                            .grow(),
                    );
                    button(
                        p,
                        cx,
                        Btn::new("Delete key", Action::Anim(AnimOp::DeleteKey)).grow(),
                    );
                });
                hint(
                    p,
                    cx,
                    "Editing a value creates or updates this bone's key at the playhead.",
                );
            });
        }
        _ => {
            section(p, cx, "character", "Character", |p| {
                row(p, cx, "Body", |p| {
                    dropdown(
                        p,
                        cx,
                        Menu::Body,
                        &body.map_or("Choose…".into(), |b| b.name.clone()),
                        Some((kind_glyph(DocKind::Body), t.kind(DocKind::Body))),
                    );
                });
                if let Some(b) = body {
                    button(
                        p,
                        cx,
                        Btn::new(
                            "Open body",
                            Action::Open(DocRef {
                                kind: DocKind::Body,
                                id: b.id,
                            }),
                        )
                        .icon("↗")
                        .ghost(),
                    );
                }
                hint(
                    p,
                    cx,
                    "Changing the body clears equipment and animations, which belong to its bones.",
                );
            });
            section(p, cx, "equip", "Equip a prop", |p| {
                let prop = menus::equip_prop(e);
                row(p, cx, "Prop", |p| {
                    dropdown(
                        p,
                        cx,
                        Menu::EquipProp,
                        &prop.map_or("No props".into(), |v| v.name.clone()),
                        Some((kind_glyph(DocKind::Prop), t.kind(DocKind::Prop))),
                    );
                });
                let anchor = menus::equip_anchor(e)
                    .and_then(|id| prop?.anchors.iter().find(|a| a.id == id))
                    .map_or("No anchors".into(), |a| a.name.clone());
                row(p, cx, "Anchor", |p| {
                    dropdown(p, cx, Menu::EquipAnchor, &anchor, Some(("✦", t.warning)))
                });
                let mount = menus::equip_mount(e)
                    .and_then(|id| body?.mounts.iter().find(|m| m.id == id))
                    .map_or("No mounts".into(), |m| m.name.clone());
                row(p, cx, "Mount", |p| {
                    dropdown(p, cx, Menu::EquipMount, &mount, Some(("✦", t.warning)))
                });
                button(
                    p,
                    cx,
                    Btn::new("Attach", Action::Attach).icon("+").primary(),
                );
            });
            section(p, cx, "animations", "Animations", |p| {
                readout(p, cx, "Clips", c.animations.len().to_string());
                hstack(p, 6., |p| {
                    button(
                        p,
                        cx,
                        Btn::new("New clip", Action::Anim(AnimOp::New))
                            .icon("+")
                            .grow(),
                    );
                    button(
                        p,
                        cx,
                        Btn::new("Add locomotion", Action::Anim(AnimOp::Presets))
                            .grow()
                            .tip("Standing, idle, walking and running clips for humanoid bones"),
                    );
                });
            });
        }
    }
}

fn tile(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let Some(tile) = e.active_tile() else { return };
    match e.sel() {
        Sel::Socket(i) => {
            if let Some(s) = tile.sockets.get(i) {
                section(p, cx, "socket", "Socket", |p| {
                    row(p, cx, "Name", |p| {
                        text_input(p, cx, TextTarget::SocketName, &s.name, "Name", true)
                    });
                    row(p, cx, "Type", |p| {
                        dropdown(p, cx, Menu::SocketKind, &s.kind, Some(("○", t.socket)))
                    });
                    row(p, cx, "Profile", |p| {
                        text_input(
                            p,
                            cx,
                            TextTarget::SocketProfile,
                            &s.profile,
                            "Profile",
                            true,
                        )
                    });
                    vector(
                        p,
                        cx,
                        "Position",
                        [
                            Field::SocketPosition(0),
                            Field::SocketPosition(1),
                            Field::SocketPosition(2),
                        ],
                    );
                    let facing = Quat::from_array(s.rotation) * Vec3::Z;
                    row(p, cx, "Facing", |p| {
                        p.spawn(text(
                            cx,
                            format!("{:.0}°", facing.x.atan2(facing.z).to_degrees()),
                            size::MD,
                            t.text,
                            Weight::Mono,
                        ));
                        spacer(p);
                        button(p, cx, Btn::new("Turn 90°", Action::TurnSocket).icon("⟳"));
                    });
                    row(p, cx, "Rules", |p| {
                        checkbox(p, cx, "Required", s.required, Action::ToggleRequired);
                    });
                    hint(
                        p,
                        cx,
                        "Positions are in voxel units (0.1). Sockets connect when types are paired in Socket rules and profiles match.",
                    );
                    button(
                        p,
                        cx,
                        Btn::new("Delete socket", Action::DeleteSelection)
                            .icon("✕")
                            .danger(),
                    );
                });
            }
        }
        Sel::Collider(i) if tile.colliders.get(i).is_some() => {
            section(p, cx, "collider", "Collision box", |p| {
                vector(
                    p,
                    cx,
                    "Min",
                    [
                        Field::ColliderMin(0),
                        Field::ColliderMin(1),
                        Field::ColliderMin(2),
                    ],
                );
                vector(
                    p,
                    cx,
                    "Max",
                    [
                        Field::ColliderMax(0),
                        Field::ColliderMax(1),
                        Field::ColliderMax(2),
                    ],
                );
                button(
                    p,
                    cx,
                    Btn::new("Delete box", Action::DeleteSelection)
                        .icon("✕")
                        .danger(),
                );
            });
        }
        _ => {}
    }
    section(p, cx, "tile", "Tile", |p| {
        row(p, cx, "Category", |p| {
            text_input(
                p,
                cx,
                TextTarget::TileCategory,
                &tile.category,
                "Category",
                true,
            )
        });
        readout(p, cx, "Volumes", tile.voxels.len().to_string());
        readout(
            p,
            cx,
            "Rotations",
            format!("{} allowed", tile.rotations.len()),
        );
        let used = e
            .library
            .worlds
            .iter()
            .map(|w| w.instances.iter().filter(|i| i.tile == tile.id).count())
            .sum::<usize>();
        readout(p, cx, "Placed", format!("{used} times"));
    });
    voxel_tool(p, cx, DocKind::Tile);
    section(p, cx, "tile-sockets", "Sockets & collision", |p| {
        hstack(p, 6., |p| {
            button(
                p,
                cx,
                Btn::new("Add socket", Action::AddSocket)
                    .icon("○")
                    .grow()
                    .tip("Add a socket at the cursor"),
            );
            button(
                p,
                cx,
                Btn::new("Add box", Action::AddCollider)
                    .icon("▣")
                    .grow()
                    .tip("Add a collision box at the cursor, sized by the brush"),
            );
        });
        hint(p, cx, "New sockets and boxes start at the voxel cursor.");
    });
}

fn world(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let Some(w) = e.active_world() else { return };
    let sel = e.sel();
    match sel {
        Sel::Piece(id) | Sel::PieceSocket(id, _) => {
            if let Some(i) = w.instances.iter().find(|v| v.id == id) {
                let tile = tiles::tile(&e.library.tiles, i.tile).ok();
                section(p, cx, "piece", "Piece", |p| {
                    readout(
                        p,
                        cx,
                        "Tile",
                        tile.map_or("missing".into(), |t| t.name.clone()),
                    );
                    readout(p, cx, "Instance", format!("#{}", i.id));
                    readout(
                        p,
                        cx,
                        "Position",
                        format!(
                            "{:.1}, {:.1}, {:.1}",
                            i.position[0], i.position[1], i.position[2]
                        ),
                    );
                    if let Some(tile) = tile {
                        let open = (0..tile.sockets.len())
                            .filter(|n| {
                                !w.connected(tiles::Port {
                                    instance: i.id,
                                    socket: *n,
                                })
                            })
                            .count();
                        readout(
                            p,
                            cx,
                            "Sockets",
                            format!("{} open of {}", open, tile.sockets.len()),
                        );
                        hstack(p, 6., |p| {
                            button(
                                p,
                                cx,
                                Btn::new(
                                    "Open tile",
                                    Action::Open(DocRef {
                                        kind: DocKind::Tile,
                                        id: tile.id,
                                    }),
                                )
                                .icon("↗")
                                .grow(),
                            );
                            button(
                                p,
                                cx,
                                Btn::new("Remove", Action::RemovePiece)
                                    .icon("✕")
                                    .danger()
                                    .grow(),
                            );
                        });
                        let required_open = tile.sockets.iter().enumerate().any(|(n, s)| {
                            s.required
                                && !w.connected(tiles::Port {
                                    instance: i.id,
                                    socket: n,
                                })
                        });
                        if required_open {
                            button(
                                p,
                                cx,
                                Btn::new("Complete piece", Action::CompletePiece)
                                    .icon("✦")
                                    .primary()
                                    .tip("Fill this piece's required sockets (and theirs) with fitting tiles"),
                            );
                        }
                    }
                });
            }
            if let Sel::PieceSocket(_, _) = sel {
                attach_section(p, cx);
            }
        }
        _ => {
            if w.instances.is_empty() {
                section(p, cx, "start", "First piece", |p| {
                    placement_controls(p, cx);
                    hint(
                        p,
                        cx,
                        "The first tile is placed at the origin. Then click open sockets to grow the world.",
                    );
                });
            }
        }
    }
    section(p, cx, "world", "World", |p| {
        readout(p, cx, "Pieces", w.instances.len().to_string());
        readout(p, cx, "Connections", w.connections.len().to_string());
        readout(p, cx, "Seed", w.seed.to_string());
        hstack(p, 6., |p| {
            button(
                p,
                cx,
                Btn::new("Validate", Action::Validate).icon("✓").grow(),
            );
            button(
                p,
                cx,
                Btn::new("Play world", Action::Play)
                    .icon("▶")
                    .tone(Tone::Success)
                    .grow(),
            );
        });
    });
    section(p, cx, "generate", "Generate", |p| {
        if e.library.tiles.is_empty() {
            hint(p, cx, "World generation needs a tile kit.");
            button(
                p,
                cx,
                Btn::new("Install starter kit", Action::InstallStarterKit).primary(),
            );
            return;
        }
        row(p, cx, "Seed", |p| number(p, cx, Field::Seed, None, true));
        row(p, cx, "Size", |p| {
            number(p, cx, Field::WorldSize, None, true)
        });
        row(p, cx, "Houses", |p| {
            number(p, cx, Field::Houses, None, true)
        });
        row(p, cx, "Planting", |p| {
            number(p, cx, Field::Plants, None, true)
        });
        button(
            p,
            cx,
            Btn::new("Generate new world", Action::Generate)
                .icon("✦")
                .primary(),
        );
        hint(
            p,
            cx,
            "Builds a neighbourhood from the starter kit (hill, stream, streets, lots and parks) as a new world document. Houses sets the number of building lots.",
        );
    });
    let _ = t;
}

fn placement_controls(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let tile = e
        .placement
        .tile
        .and_then(|id| tiles::tile(&e.library.tiles, id).ok());
    row(p, cx, "Tile", |p| {
        dropdown(
            p,
            cx,
            Menu::PlaceTile,
            &tile.map_or("Choose a tile…".into(), |t| t.name.clone()),
            Some((kind_glyph(DocKind::Tile), t.kind(DocKind::Tile))),
        );
    });
    if let Some(tile) = tile {
        let n = tile.rotations.len().max(1);
        row(p, cx, "Rotation", |p| {
            p.spawn(text(
                cx,
                format!("{}°", e.placement.turn % n * 360 / n),
                size::MD,
                t.text,
                Weight::Mono,
            ));
            spacer(p);
            button(p, cx, Btn::new("Rotate", Action::TurnPlacement).icon("⟳"));
        });
    }
    let candidate = world::candidate(e);
    match &candidate {
        Ok(_) => status(p, cx, true, "Fits — the green box shows where it goes."),
        Err(err) if tile.is_some() => status(p, cx, false, err.clone()),
        Err(_) => {}
    }
    button(
        p,
        cx,
        Btn::new("Place tile", Action::Place)
            .icon("+")
            .primary()
            .enabled(candidate.is_ok()),
    );
}

fn attach_section(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let Some(port) = world::target_port(e) else {
        return;
    };
    let Some(w) = e.active_world() else { return };
    let Ok((_, socket)) = tiles::port(w, &e.library.tiles, port) else {
        return;
    };
    section(p, cx, "socket-attach", "Socket", |p| {
        readout(p, cx, "Name", socket.name.clone());
        readout(
            p,
            cx,
            "Type",
            format!("{} · {}", socket.kind, socket.profile),
        );
        readout(
            p,
            cx,
            "Required",
            if socket.required { "yes" } else { "no" },
        );
        if w.connected(port) {
            status(p, cx, true, "Connected.");
            return;
        }
        let options = world::options(e);
        readout(p, cx, "Fits", format!("{} tile options", options.len()));
        placement_controls(p, cx);
    });
}

fn rules(p: Kids, cx: &Ctx) {
    let e = cx.e;
    section(p, cx, "rules", "Rules", |p| {
        readout(
            p,
            cx,
            "Types",
            actions::socket_kinds(&e.library).len().to_string(),
        );
        readout(
            p,
            cx,
            "Pairs",
            e.library.socket_rules.pairs.len().to_string(),
        );
        if let Some((a, b)) = &e.rule_focus {
            readout(p, cx, "Last edit", format!("{a} ↔ {b}"));
            status(
                p,
                cx,
                e.library.socket_rules.allows(a, b),
                if e.library.socket_rules.allows(a, b) {
                    "These socket types may connect."
                } else {
                    "These socket types may not connect."
                },
            );
        }
        hint(
            p,
            cx,
            "Click a cell in the matrix to toggle a pair. A tile cannot be placed if another of its unused sockets would line up with an incompatible one.",
        );
    });
}
