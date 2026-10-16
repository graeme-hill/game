//! Screens shown before anything is open: the welcome card and the empty
//! editor with quick actions and shortcuts.
use super::theme::{kind_glyph, size};
use super::widgets::*;
use super::*;

pub fn no_document(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    p.spawn(Node {
        flex_grow: 1.,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|p| {
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(14),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|p| {
            p.spawn(text(cx, "Nothing open", size::XL, t.text, Weight::Semibold));
            p.spawn(text(
                cx,
                "Pick a resource in the Explorer, or create something new.",
                size::MD,
                t.text_dim,
                Weight::Regular,
            ));
            p.spawn(Node {
                column_gap: px(8),
                margin: UiRect::top(px(6)),
                ..default()
            })
            .with_children(|p| {
                for kind in [
                    DocKind::Body,
                    DocKind::Prop,
                    DocKind::Character,
                    DocKind::World,
                ] {
                    button(
                        p,
                        cx,
                        Btn::new(
                            format!("New {}", kind.label().to_lowercase()),
                            Action::New(kind),
                        )
                        .icon(kind_glyph(kind))
                        .icon_color(t.kind(kind)),
                    );
                }
            });
            shortcuts(p, cx);
        });
    });
}

fn shortcuts(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    p.spawn(Node {
        display: Display::Grid,
        grid_template_columns: vec![RepeatedGridTrack::auto(2), RepeatedGridTrack::auto(2)],
        column_gap: px(14),
        row_gap: px(6),
        margin: UiRect::top(px(18)),
        ..default()
    })
    .with_children(|p| {
        for (keys, what) in [
            ("Ctrl+S", "Save all"),
            ("Ctrl+Z / Ctrl+Y", "Undo / redo"),
            ("Ctrl+N", "New resource"),
            ("F2", "Rename"),
            ("F", "Frame selection"),
            ("Del", "Delete selection"),
            ("RMB drag", "Orbit"),
            ("F5", "Play"),
        ] {
            p.spawn((
                Node {
                    padding: UiRect::axes(px(6), px(2)),
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(4)),
                    justify_self: JustifySelf::End,
                    ..default()
                },
                BackgroundColor(t.raised),
                BorderColor::all(t.border),
                children![text(cx, keys, size::SM, t.text_dim, Weight::Mono)],
            ));
            p.spawn(text(cx, what, size::SM, t.text_muted, Weight::Regular));
        }
    });
}

pub fn welcome(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    let e = cx.e;
    p.spawn(Node {
        flex_grow: 1.,
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        ..default()
    })
    .with_children(|p| {
        p.spawn((
            Node {
                width: px(520),
                flex_direction: FlexDirection::Column,
                row_gap: px(14),
                padding: UiRect::all(px(28)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(12)),
                ..default()
            },
            BackgroundColor(t.panel),
            BorderColor::all(t.border),
            BoxShadow::new(t.shadow, px(0), px(10), px(0), px(30)),
        ))
        .with_children(|p| {
            p.spawn(Node {
                column_gap: px(12),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|p| {
                p.spawn((
                    Node {
                        width: px(44),
                        height: px(44),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        border_radius: BorderRadius::all(px(10)),
                        ..default()
                    },
                    BackgroundGradient::from(LinearGradient::to_bottom_right(vec![
                        t.accent.into(),
                        t.kind(DocKind::Body).into(),
                    ])),
                    children![glyph(cx, "◆", 22., Color::WHITE)],
                ));
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|p| {
                    p.spawn(text(cx, "Welcome to Workshop", size::HERO * 0.8, t.text, Weight::Semibold));
                    p.spawn(text(
                        cx,
                        "Design bodies, props, tiles and worlds — then play them.",
                        size::MD,
                        t.text_dim,
                        Weight::Regular,
                    ));
                });
            });
            divider(p, cx);
            if e.notice.kind == NoticeKind::Error {
                status(p, cx, false, e.notice.text.clone());
            }
            p.spawn(text(cx, "OPEN A WORKSPACE", size::XS, t.text_muted, Weight::Semibold));
            hint(
                p,
                cx,
                "A workspace is a folder of .body, .prop, .character, .tile and .world source files.",
            );
            p.spawn((
                Node {
                    padding: UiRect::axes(px(10), px(8)),
                    border_radius: BorderRadius::all(px(6)),
                    column_gap: px(8),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(t.field),
            ))
            .with_children(|p| {
                p.spawn(glyph(cx, "▤", 13., t.text_dim));
                p.spawn(text(
                    cx,
                    ellipsize(&e.workspace_dir.display().to_string(), 60),
                    size::MD,
                    t.text,
                    Weight::Mono,
                ));
            });
            p.spawn(Node {
                column_gap: px(8),
                ..default()
            })
            .with_children(|p| {
                button(
                    p,
                    cx,
                    Btn::new("Open this folder", Action::OpenWorkspace(e.workspace_dir.clone()))
                        .primary()
                        .grow(),
                );
                button(
                    p,
                    cx,
                    Btn::new("Browse…", Action::ShowWorkspaceBrowser).grow(),
                );
            });
            let recent: Vec<_> = e.recent.iter().filter(|p| **p != e.workspace_dir).collect();
            if !recent.is_empty() {
                p.spawn(text(cx, "RECENT", size::XS, t.text_muted, Weight::Semibold));
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|p| {
                    for path in recent.into_iter().take(5) {
                        recent_row(p, cx, path);
                    }
                });
            }
        });
    });
}

/// A recently opened workspace: folder name and its full path.
pub fn recent_row(p: Kids, cx: &Ctx, path: &std::path::Path) {
    let t = &cx.t;
    let name = path.file_name().map_or(path.display().to_string(), |n| {
        n.to_string_lossy().into_owned()
    });
    p.spawn((
        Button,
        control(
            Action::OpenWorkspace(path.to_path_buf()),
            name.clone(),
            ControlKind::Button,
        ),
        Node {
            height: px(30),
            padding: UiRect::horizontal(px(8)),
            column_gap: px(10),
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(5)),
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
        p.spawn(text(
            cx,
            ellipsize(&name, 24),
            size::MD,
            t.text,
            Weight::Medium,
        ));
        p.spawn(text(
            cx,
            ellipsize(&path.display().to_string(), 44),
            size::SM,
            t.text_muted,
            Weight::Regular,
        ));
    });
}
