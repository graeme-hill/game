//! Floating layers: popup menus and modal dialogs.
use super::theme::{kind_glyph, size};
use super::widgets::*;
use super::*;

pub fn build(root: Kids, cx: &Ctx) {
    if let Some(open) = &cx.e.menu {
        popup(root, cx, open);
    }
    if let Some(modal) = &cx.e.modal {
        match modal {
            Modal::Workspace { dir } => workspace_browser(root, cx, dir),
            Modal::Unsaved(pending) => unsaved(root, cx, pending),
            Modal::Shortcuts => shortcuts(root, cx),
        }
    }
}

fn popup(root: Kids, cx: &Ctx, open: &OpenMenu) {
    let t = &cx.t;
    let items = menus::items(cx.e, &open.menu);
    // Keep the whole menu inside the window: shift it up or left if needed.
    let long = items.len() > 14;
    let height = if long {
        420.
    } else {
        items.len() as f32 * 26. + items.iter().filter(|i| i.separator).count() as f32 * 9. + 10.
    };
    let width = open.width.max(200.);
    let left = open.pos.x.min(cx.size.x - width - 6.).max(4.);
    let top = open.pos.y.min(cx.size.y - height - 6.).max(4.);
    root.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(top),
            min_width: px(open.width),
            max_height: px((cx.size.y - 12.).min(420.)),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(px(4)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(7)),
            ..default()
        },
        BackgroundColor(t.raised),
        BorderColor::all(t.border),
        BoxShadow::new(t.shadow, px(0), px(8), px(0), px(24)),
        GlobalZIndex(50),
    ))
    .with_children(|p| {
        if items.is_empty() {
            p.spawn((
                Node {
                    padding: UiRect::axes(px(10), px(6)),
                    ..default()
                },
                children![text(
                    cx,
                    "Nothing to choose",
                    size::MD,
                    t.text_muted,
                    Weight::Regular
                )],
            ));
            return;
        }
        let body = |p: Kids| {
            for item in items {
                if item.separator {
                    p.spawn((
                        Node {
                            height: px(1),
                            margin: UiRect::axes(px(6), px(4)),
                            flex_shrink: 0.,
                            ..default()
                        },
                        BackgroundColor(t.border),
                    ));
                }
                let destructive = matches!(item.action, Action::Delete(_));
                p.spawn((
                    Button,
                    control(item.action, item.label.clone(), ControlKind::MenuItem),
                    Node {
                        height: px(26),
                        flex_shrink: 0.,
                        padding: UiRect::horizontal(px(8)),
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(px(4)),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    HoverStyle {
                        idle: Color::NONE,
                        hover: if destructive {
                            t.danger_dim
                        } else {
                            t.accent_dim
                        },
                        press: t.press,
                    },
                ))
                .with_children(|p| {
                    p.spawn((
                        Node {
                            width: px(14),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        children![glyph(
                            cx,
                            if item.checked {
                                "✓"
                            } else {
                                item.icon.map_or("", kind_glyph)
                            },
                            11.,
                            if item.checked {
                                t.accent_text
                            } else {
                                item.icon.map_or(t.text_dim, |k| t.kind(k))
                            },
                        )],
                    ));
                    p.spawn(text(
                        cx,
                        ellipsize(&item.label, 30),
                        size::MD,
                        if destructive { t.danger } else { t.text },
                        Weight::Regular,
                    ));
                    spacer(p);
                    if !item.hint.is_empty() {
                        p.spawn(text(cx, item.hint, size::SM, t.text_muted, Weight::Regular));
                    }
                });
            }
        };
        if long {
            p.spawn(Node {
                height: px(410),
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|p| scroll(p, cx, "popup", UiRect::right(px(6)), 0., body));
        } else {
            body(p);
        }
    });
}

const SHORTCUTS: &[(&str, &[(&str, &str)])] = &[
    (
        "General",
        &[
            ("Ctrl+S", "Save all"),
            ("Ctrl+Z", "Undo"),
            ("Ctrl+Y / Ctrl+Shift+Z", "Redo"),
            ("Ctrl+N", "New resource"),
            ("Ctrl+O", "Open workspace"),
            ("Ctrl+Q", "Quit"),
            ("Ctrl+W / middle-click", "Close tab"),
            ("Ctrl+Tab", "Next tab"),
            ("F5", "Save and play"),
            ("F1 / ?", "This sheet"),
        ],
    ),
    (
        "Selection",
        &[
            ("↑ ↓", "Move through the Outline"),
            ("F2 / double-click", "Rename"),
            ("Ctrl+D", "Duplicate selection"),
            ("Del", "Delete selection"),
            ("Esc", "Deselect / close"),
            ("Right-click", "Actions for an item"),
        ],
    ),
    (
        "Viewport",
        &[
            ("Right-drag", "Orbit"),
            ("Middle / Shift+right-drag", "Pan"),
            ("Wheel", "Zoom"),
            ("F", "Frame"),
            ("1 3 7 0", "Front, side, top, perspective"),
            ("G", "Guides"),
            ("\\", "Hide panels"),
            ("Ctrl+drag handle", "Snap to 0.1"),
        ],
    ),
    (
        "Voxels and animation",
        &[
            ("B / E / V", "Paint, erase, select"),
            ("[ ]", "Brush size"),
            ("M", "Mirror across X"),
            ("Alt+click", "Pick a voxel's color"),
            ("Space", "Play / pause"),
            (", .", "Previous / next key"),
            ("← →", "Step one frame"),
            ("K", "Key the selected bone"),
        ],
    ),
];

fn shortcuts(root: Kids, cx: &Ctx) {
    let t = &cx.t;
    root.spawn((
        UiBlocker,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(t.scrim),
        GlobalZIndex(40),
    ))
    .with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: px(680),
                    max_width: percent(96),
                    max_height: percent(94),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(10)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(t.panel),
                BorderColor::all(t.border),
                BoxShadow::new(t.shadow, px(0), px(14), px(0), px(40)),
            ))
            .with_children(|panel| {
                panel
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(16), px(12)),
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(px(1)),
                            ..default()
                        },
                        BorderColor::all(t.divider),
                    ))
                    .with_children(|p| {
                        p.spawn(text(
                            cx,
                            "Keyboard shortcuts",
                            size::LG,
                            t.text,
                            Weight::Semibold,
                        ));
                        spacer(p);
                        icon_button(p, cx, "✕", Action::CloseModal, "Close (Esc)");
                    });
                panel
                    .spawn(Node {
                        display: Display::Grid,
                        grid_template_columns: RepeatedGridTrack::flex(2, 1.),
                        column_gap: px(24),
                        row_gap: px(16),
                        padding: UiRect::all(px(16)),
                        ..default()
                    })
                    .with_children(|grid| {
                        for (title, keys) in SHORTCUTS {
                            grid.spawn(Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: px(5),
                                ..default()
                            })
                            .with_children(|p| {
                                p.spawn(text(
                                    cx,
                                    title.to_uppercase(),
                                    size::XS,
                                    t.text_muted,
                                    Weight::Semibold,
                                ));
                                for (key, what) in keys.iter() {
                                    p.spawn(Node {
                                        column_gap: px(10),
                                        align_items: AlignItems::Center,
                                        ..default()
                                    })
                                    .with_children(|p| {
                                        p.spawn(text(
                                            cx,
                                            *what,
                                            size::MD,
                                            t.text_dim,
                                            Weight::Regular,
                                        ));
                                        spacer(p);
                                        p.spawn((
                                            Node {
                                                padding: UiRect::axes(px(6), px(1)),
                                                border: UiRect::all(px(1)),
                                                border_radius: BorderRadius::all(px(4)),
                                                ..default()
                                            },
                                            BackgroundColor(t.raised),
                                            BorderColor::all(t.border),
                                            children![text(
                                                cx,
                                                *key,
                                                size::SM,
                                                t.text,
                                                Weight::Mono
                                            )],
                                        ));
                                    });
                                }
                            });
                        }
                    });
            });
    });
}

/// "Save changes?" before quitting or switching workspaces.
fn unsaved(root: Kids, cx: &Ctx, pending: &Pending) {
    let t = &cx.t;
    let e = cx.e;
    let dirty: Vec<DocRef> = DocKind::ALL
        .into_iter()
        .flat_map(|kind| e.docs(kind))
        .filter(|doc| e.doc_dirty(*doc))
        .collect();
    root.spawn((
        UiBlocker,
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(t.scrim),
        GlobalZIndex(40),
    ))
    .with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: px(440),
                    max_width: percent(94),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(12),
                    padding: UiRect::all(px(20)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(10)),
                    ..default()
                },
                BackgroundColor(t.panel),
                BorderColor::all(t.border),
                BoxShadow::new(t.shadow, px(0), px(14), px(0), px(40)),
            ))
            .with_children(|p| {
                p.spawn(Node {
                    column_gap: px(10),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|p| {
                    p.spawn(glyph(cx, "⚠", 18., t.warning));
                    p.spawn(text(cx, "Save your changes?", size::LG, t.text, Weight::Semibold));
                });
                p.spawn(wrapped(
                    cx,
                    match pending {
                        Pending::Quit => "These resources have unsaved changes that will be lost if you quit without saving.",
                        Pending::Open(_) => "These resources have unsaved changes that will be lost if you open another workspace without saving.",
                    },
                    size::MD,
                    t.text_dim,
                ));
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4),
                    padding: UiRect::all(px(8)),
                    border_radius: BorderRadius::all(px(6)),
                    ..default()
                })
                .insert(BackgroundColor(t.field))
                .with_children(|p| {
                    for doc in dirty.iter().take(6) {
                        p.spawn(Node {
                            column_gap: px(8),
                            align_items: AlignItems::Center,
                            ..default()
                        })
                        .with_children(|p| {
                            p.spawn(glyph(cx, kind_glyph(doc.kind), 11., t.kind(doc.kind)));
                            p.spawn(text(cx, e.doc_name(*doc), size::MD, t.text, Weight::Regular));
                        });
                    }
                    if dirty.len() > 6 {
                        p.spawn(text(cx, format!("and {} more", dirty.len() - 6), size::SM, t.text_muted, Weight::Regular));
                    }
                    if dirty.is_empty() {
                        p.spawn(text(cx, "Deleted resources", size::MD, t.text, Weight::Regular));
                    }
                });
                p.spawn(Node {
                    column_gap: px(8),
                    margin: UiRect::top(px(4)),
                    ..default()
                })
                .with_children(|p| {
                    button(p, cx, Btn::new("Don't save", Action::Resolve(false)).danger());
                    spacer(p);
                    button(p, cx, Btn::new("Cancel", Action::CloseModal));
                    button(p, cx, Btn::new("Save", Action::Resolve(true)).primary());
                });
            });
    });
}

fn workspace_browser(root: Kids, cx: &Ctx, dir: &std::path::Path) {
    let t = &cx.t;
    let mut directories: Vec<_> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .filter(|path| {
            !path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        })
        .collect();
    directories.sort();
    let resources = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| {
            matches!(
                entry.path().extension().and_then(|e| e.to_str()),
                Some("body" | "prop" | "character" | "tile" | "world" | "sockets" | "json")
            )
        })
        .count();
    root.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(t.scrim),
        GlobalZIndex(40),
    ))
    .with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: px(620),
                    height: px(460),
                    max_width: percent(94),
                    max_height: percent(92),
                    flex_direction: FlexDirection::Column,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(10)),
                    overflow: Overflow::clip(),
                    ..default()
                },
                BackgroundColor(t.panel),
                BorderColor::all(t.border),
                BoxShadow::new(t.shadow, px(0), px(14), px(0), px(40)),
            ))
            .with_children(|panel| {
                panel
                    .spawn((
                        Node {
                            padding: UiRect::axes(px(16), px(12)),
                            column_gap: px(8),
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(px(1)),
                            ..default()
                        },
                        BorderColor::all(t.divider),
                    ))
                    .with_children(|p| {
                        p.spawn(glyph(cx, "▤", 15., t.accent_text));
                        p.spawn(text(
                            cx,
                            "Open workspace",
                            size::LG,
                            t.text,
                            Weight::Semibold,
                        ));
                        spacer(p);
                        icon_button(p, cx, "✕", Action::CloseModal, "Close (Esc)");
                    });
                panel
                    .spawn(Node {
                        padding: UiRect::axes(px(16), px(10)),
                        column_gap: px(6),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|p| {
                        if let Some(parent) = dir.parent() {
                            button(
                                p,
                                cx,
                                Btn::new("", Action::Browse(parent.to_path_buf()))
                                    .icon("↑")
                                    .tip("Parent folder"),
                            );
                        }
                        let display = dir.display().to_string();
                        text_input(p, cx, TextTarget::Path, &display, "Path", true);
                    });
                if !cx.e.recent.is_empty() {
                    panel
                        .spawn(Node {
                            padding: UiRect::horizontal(px(16)),
                            margin: UiRect::bottom(px(8)),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(2),
                            ..default()
                        })
                        .with_children(|p| {
                            p.spawn(text(cx, "RECENT", size::XS, t.text_muted, Weight::Semibold));
                            for path in cx.e.recent.iter().take(3) {
                                super::start::recent_row(p, cx, path);
                            }
                        });
                }
                panel
                    .spawn(Node {
                        flex_grow: 1.,
                        min_height: px(0),
                        margin: UiRect::horizontal(px(16)),
                        flex_direction: FlexDirection::Column,
                        border: UiRect::all(px(1)),
                        border_radius: BorderRadius::all(px(6)),
                        ..default()
                    })
                    .insert((BorderColor::all(t.border), BackgroundColor(t.field)))
                    .with_children(|p| {
                        scroll(p, cx, "workspace-browser", UiRect::all(px(4)), 1., |list| {
                            if directories.is_empty() {
                                list.spawn((
                                    Node {
                                        padding: UiRect::all(px(10)),
                                        ..default()
                                    },
                                    children![text(
                                        cx,
                                        "No subfolders",
                                        size::MD,
                                        t.text_muted,
                                        Weight::Regular
                                    )],
                                ));
                            }
                            for path in directories {
                                let name =
                                    path.file_name().map_or(path.display().to_string(), |n| {
                                        n.to_string_lossy().into_owned()
                                    });
                                list.spawn((
                                    Button,
                                    control(
                                        Action::Browse(path),
                                        name.clone(),
                                        ControlKind::Button,
                                    ),
                                    Node {
                                        height: px(28),
                                        flex_shrink: 0.,
                                        padding: UiRect::horizontal(px(10)),
                                        column_gap: px(8),
                                        align_items: AlignItems::Center,
                                        border_radius: BorderRadius::all(px(4)),
                                        ..default()
                                    },
                                    BackgroundColor(Color::NONE),
                                    HoverStyle {
                                        idle: Color::NONE,
                                        hover: t.hover,
                                        press: t.press,
                                    },
                                ))
                                .with_children(|p| {
                                    p.spawn(glyph(cx, "▤", 12., t.kind(DocKind::Prop)));
                                    p.spawn(text(cx, name, size::MD, t.text, Weight::Regular));
                                    spacer(p);
                                    p.spawn(glyph(cx, "›", 13., t.text_muted));
                                });
                            }
                        });
                    });
                panel
                    .spawn(Node {
                        padding: UiRect::all(px(16)),
                        column_gap: px(8),
                        align_items: AlignItems::Center,
                        ..default()
                    })
                    .with_children(|p| {
                        p.spawn(text(
                            cx,
                            if resources > 0 {
                                format!("{resources} resource files in this folder")
                            } else {
                                "No resources here yet — a new workspace starts empty".into()
                            },
                            size::SM,
                            t.text_muted,
                            Weight::Regular,
                        ));
                        spacer(p);
                        button(p, cx, Btn::new("Cancel", Action::CloseModal));
                        button(
                            p,
                            cx,
                            Btn::new("Open folder", Action::OpenWorkspace(dir.to_path_buf()))
                                .primary(),
                        );
                    });
            });
    });
}
