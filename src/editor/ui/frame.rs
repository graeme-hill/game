//! The window frame: top bar, panels, tab strip, center area and status bar.
use super::theme::{kind_glyph, metric, size};
use super::widgets::*;
use super::*;
use crate::editor::viewport::ViewportSlot;

pub fn build(root: Kids, cx: &Ctx) {
    top_bar(root, cx);
    root.spawn(Node {
        flex_grow: 1.,
        min_height: px(0),
        ..default()
    })
    .with_children(|p| {
        if cx.e.workspace_ready && cx.e.layout.show_left {
            explorer::left_panel(p, cx);
        }
        center(p, cx);
        if cx.e.workspace_ready && cx.e.layout.show_right {
            inspector::right_panel(p, cx);
        }
    });
    status_bar(root, cx);
    overlays::build(root, cx);
}

fn top_bar(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    let e = cx.e;
    p.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            height: px(metric::TOP_BAR),
            flex_shrink: 0.,
            padding: UiRect::horizontal(px(10)),
            align_items: AlignItems::Center,
            column_gap: px(6),
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BackgroundColor(t.panel_header),
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        // Brand mark.
        p.spawn((
            Node {
                width: px(24),
                height: px(24),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(6)),
                margin: UiRect::right(px(4)),
                ..default()
            },
            BackgroundGradient::from(LinearGradient::to_bottom_right(vec![
                t.accent.into(),
                t.kind(DocKind::Body).into(),
            ])),
            children![glyph(cx, "◆", 13., Color::WHITE)],
        ));
        p.spawn(text(cx, "Workshop", size::LG, t.text, Weight::Semibold));
        vdivider(p, cx, 18.);
        button(
            p,
            cx,
            Btn::new("File", Action::OpenMenu(Menu::File))
                .ghost()
                .active(e.menu.as_ref().is_some_and(|m| m.menu == Menu::File)),
        );
        if !e.workspace_ready {
            button(
                p,
                cx,
                Btn::new("Open workspace…", Action::ShowWorkspaceBrowser)
                    .icon("▤")
                    .ghost(),
            );
        } else {
            let folder = e
                .workspace_dir
                .file_name()
                .map_or(e.workspace_dir.display().to_string(), |n| {
                    n.to_string_lossy().into_owned()
                });
            button(
                p,
                cx,
                Btn::new(ellipsize(&folder, 28), Action::ShowWorkspaceBrowser)
                    .icon("▤")
                    .ghost()
                    .tip(format!(
                        "Workspace: {} — click to open another",
                        e.workspace_dir.display()
                    )),
            );
            vdivider(p, cx, 18.);
            button(
                p,
                cx,
                Btn::new(if e.dirty() { "Save" } else { "Saved" }, Action::Save)
                    .icon(if e.dirty() { "●" } else { "✓" })
                    .icon_color(if e.dirty() { t.warning } else { t.success })
                    .ghost()
                    .tip("Save all resources (Ctrl+S)"),
            );
            button(
                p,
                cx,
                Btn::new("", Action::Undo)
                    .icon("↶")
                    .ghost()
                    .enabled(e.can_undo())
                    .tip(
                        e.undo_label()
                            .map_or("Nothing to undo".into(), |l| format!("Undo {l} (Ctrl+Z)")),
                    ),
            );
            button(
                p,
                cx,
                Btn::new("", Action::Redo)
                    .icon("↷")
                    .ghost()
                    .enabled(e.can_redo())
                    .tip(
                        e.redo_label()
                            .map_or("Nothing to redo".into(), |l| format!("Redo {l} (Ctrl+Y)")),
                    ),
            );
        }
        spacer(p);
        if let Some(doc) = e.active {
            p.spawn(glyph(cx, kind_glyph(doc.kind), 12., t.kind(doc.kind)));
            p.spawn(text(
                cx,
                doc.kind.plural(),
                size::MD,
                t.text_muted,
                Weight::Regular,
            ));
            p.spawn(glyph(cx, "›", 13., t.text_muted));
            p.spawn(text(
                cx,
                ellipsize(&e.doc_name(doc), 32),
                size::MD,
                t.text,
                Weight::Medium,
            ));
        }
        spacer(p);
        button(
            p,
            cx,
            Btn::new("", Action::ShowShortcuts)
                .icon("?")
                .ghost()
                .tip("Keyboard shortcuts (F1)"),
        );
        if e.workspace_ready {
            button(
                p,
                cx,
                Btn::new("New", Action::OpenMenu(Menu::New))
                    .icon("+")
                    .active(e.menu.as_ref().is_some_and(|m| m.menu == Menu::New))
                    .tip("Create a resource (Ctrl+N)"),
            );
        }
        button(
            p,
            cx,
            Btn::new("Play", Action::Play)
                .icon("▶")
                .tone(Tone::Success)
                .tip(match e.active_kind() {
                    Some(DocKind::World) => "Save and play this world (F5)",
                    _ => "Save and play the workspace (F5)",
                }),
        );
    });
}

fn center(p: Kids, cx: &Ctx) {
    let e = cx.e;
    let t = &cx.t;
    let viewport = e.workspace_ready && e.active_kind().is_some_and(|k| k != DocKind::Rules);
    p.spawn((
        Node {
            flex_grow: 1.,
            flex_basis: px(0),
            min_width: px(0),
            flex_direction: FlexDirection::Column,
            ..default()
        },
        BackgroundColor(if viewport { Color::NONE } else { t.bg }),
    ))
    .with_children(|p| {
        if !e.workspace_ready {
            super::start::welcome(p, cx);
            return;
        }
        tabs(p, cx);
        match e.active_kind() {
            None => super::start::no_document(p, cx),
            Some(DocKind::Rules) => super::rules::rules_matrix(p, cx),
            Some(kind) => {
                p.spawn((
                    ViewportSlot,
                    Node {
                        flex_grow: 1.,
                        min_height: px(0),
                        position_type: PositionType::Relative,
                        ..default()
                    },
                ))
                .with_children(|p| viewport_overlays(p, cx, kind));
                if kind == DocKind::Character && e.anim.clip.is_some() {
                    super::timeline::timeline(p, cx);
                }
            }
        }
    });
}

fn tabs(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    let e = cx.e;
    p.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            height: px(metric::TAB_BAR),
            flex_shrink: 0.,
            align_items: AlignItems::End,
            padding: UiRect::left(px(4)),
            column_gap: px(1),
            border: UiRect::bottom(px(1)),
            overflow: Overflow::clip_x(),
            ..default()
        },
        BackgroundColor(t.panel_header),
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        // Names shorten as more tabs share the strip.
        let per_tab = (cx.center_width() - 8.) / e.tabs.len().max(1) as f32;
        let chars = ((per_tab - 64.) / 7.).clamp(4., 22.) as usize;
        for doc in &e.tabs {
            let active = e.active == Some(*doc);
            let (bg, fg) = if active {
                (t.viewport, t.text)
            } else {
                (Color::NONE, t.text_dim)
            };
            let mut c = control(Action::Open(*doc), e.doc_name(*doc), ControlKind::Button);
            c.context = Some(Menu::Tab(*doc));
            c.middle = Some(Action::CloseTab(*doc));
            c.tooltip = format!("{} · {}", doc.kind.label(), e.doc_name(*doc));
            p.spawn((
                Button,
                c,
                Node {
                    height: px(metric::TAB_BAR - 4.),
                    padding: UiRect {
                        left: px(10),
                        right: px(4),
                        ..default()
                    },
                    align_items: AlignItems::Center,
                    column_gap: px(7),
                    border: UiRect::top(px(2)),
                    border_radius: BorderRadius::top(px(5)),
                    flex_shrink: 0.,
                    ..default()
                },
                BackgroundColor(bg),
                BorderColor::all(if active { t.accent } else { Color::NONE }),
                HoverStyle {
                    idle: bg,
                    hover: if active { bg } else { t.hover.with_alpha(0.6) },
                    press: bg,
                },
            ))
            .with_children(|p| {
                p.spawn(glyph(cx, kind_glyph(doc.kind), 11., t.kind(doc.kind)));
                p.spawn(text(
                    cx,
                    ellipsize(&e.doc_name(*doc), chars),
                    size::MD,
                    fg,
                    if active {
                        Weight::Medium
                    } else {
                        Weight::Regular
                    },
                ));
                let dirty = e.doc_dirty(*doc);
                p.spawn((
                    Button,
                    control(Action::CloseTab(*doc), "Close tab", ControlKind::Button),
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
                    children![glyph(
                        cx,
                        if dirty { "●" } else { "✕" },
                        if dirty { 9. } else { 10. },
                        if dirty { t.text_dim } else { t.text_muted },
                    )],
                ));
            });
        }
    });
}

/// Floating tool palette and view controls over the 3D viewport.
fn viewport_overlays(p: Kids, cx: &Ctx, kind: DocKind) {
    let t = &cx.t;
    let e = cx.e;
    let tool = e.tool();
    let float = |left: Option<f32>, right: Option<f32>, top: Option<f32>, bottom: Option<f32>| {
        (
            UiBlocker,
            Interaction::default(),
            Node {
                position_type: PositionType::Absolute,
                left: left.map_or(Val::Auto, px),
                right: right.map_or(Val::Auto, px),
                top: top.map_or(Val::Auto, px),
                bottom: bottom.map_or(Val::Auto, px),
                padding: UiRect::all(px(4)),
                column_gap: px(4),
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                ..default()
            },
            BackgroundColor(t.panel.with_alpha(0.92)),
            BorderColor::all(t.border),
            BoxShadow::new(t.shadow, px(0), px(4), px(0), px(12)),
        )
    };
    // Tools.
    if matches!(kind, DocKind::Prop | DocKind::Tile) {
        p.spawn(float(Some(10.), None, Some(10.), None))
            .with_children(|p| {
                let mut tools = vec![];
                if kind == DocKind::Prop {
                    tools.push(("☰".to_string(), Tool::Select, "Select anchors (V)"));
                }
                tools.push(("✎".to_string(), Tool::Paint, "Paint voxels (B)"));
                tools.push(("⌫".to_string(), Tool::Erase, "Erase voxels (E)"));
                if kind == DocKind::Tile {
                    tools.push(("⊕".to_string(), Tool::Socket, "Sockets"));
                    tools.push(("▣".to_string(), Tool::Collider, "Collision boxes"));
                }
                segmented(
                    p,
                    cx,
                    tools
                        .into_iter()
                        .map(|(g, tl, tip)| (g, Action::SetTool(tl), tool == tl, tip.to_string()))
                        .collect(),
                    false,
                );
                vdivider(p, cx, 18.);
                p.spawn(text(cx, "Brush", size::SM, t.text_muted, Weight::Regular));
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
                                format!("{b}×{b}×{b} brush ([ and ])"),
                            )
                        })
                        .collect(),
                    false,
                );
                vdivider(p, cx, 18.);
                button(
                    p,
                    cx,
                    Btn::new("", Action::ToggleMirror)
                        .icon("⇄")
                        .ghost()
                        .active(e.mirror)
                        .tip("Mirror painting across X (M)"),
                );
                swatch(p, cx, e.color, 18.);
                if kind == DocKind::Tile {
                    vdivider(p, cx, 18.);
                    icon_button(
                        p,
                        cx,
                        "⟳",
                        Action::TurnPreview,
                        "Preview the next allowed rotation",
                    );
                }
            });
    }
    // View controls.
    p.spawn(float(None, Some(10.), Some(10.), None))
        .with_children(|p| {
            icon_button(p, cx, "◎", Action::Frame, "Frame all (F)");
            button(
                p,
                cx,
                Btn::new("", Action::ToggleGuides)
                    .icon("▦")
                    .ghost()
                    .active(e.guides)
                    .tip("Show guides (G)"),
            );
            button(
                p,
                cx,
                Btn::new("View", Action::OpenMenu(Menu::View))
                    .ghost()
                    .active(e.menu.as_ref().is_some_and(|m| m.menu == Menu::View)),
            );
        });
    // Navigation hint.
    let hint = match (kind, tool) {
        (DocKind::Prop | DocKind::Tile, Tool::Paint) => {
            "Click or drag to paint · RMB orbit · MMB pan · Wheel zoom"
        }
        (DocKind::Prop | DocKind::Tile, Tool::Erase) => {
            "Click or drag to erase · RMB orbit · MMB pan · Wheel zoom"
        }
        (DocKind::World, _) => {
            "Click a piece to select · click a glowing socket to attach a tile · drag to orbit"
        }
        (DocKind::Body, _) => {
            "Click a bone or mount to select · drag to orbit · MMB pan · Wheel zoom"
        }
        (DocKind::Character, _) => {
            "Click a bone to pose · click a mount to equip there · drag to orbit"
        }
        _ => "Drag to orbit · MMB pan · Wheel zoom",
    };
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(12),
            bottom: px(10),
            ..default()
        },
        children![text(
            cx,
            hint,
            size::SM,
            t.text_dim.with_alpha(0.8),
            Weight::Regular
        )],
    ));
    // Empty-document guidance floats in the middle of the viewport.
    let empty = match kind {
        DocKind::Body => e.active_body().is_some_and(|b| b.bones.is_empty()),
        DocKind::Prop => {
            e.active_prop().is_some_and(|p| p.blocks.is_empty()) && tool == Tool::Paint
        }
        DocKind::World => e.active_world().is_some_and(|w| w.instances.is_empty()),
        _ => false,
    };
    if empty {
        p.spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(54),
            justify_content: JustifyContent::Center,
            ..default()
        })
        .with_children(|p| {
            let (title, body) = match kind {
                DocKind::Body => (
                    "Empty armature",
                    "Add a root bone, or start from the humanoid template.",
                ),
                DocKind::Prop => ("Empty prop", "Click the grid to paint your first voxels."),
                _ => (
                    "Empty world",
                    "Choose a starting tile in the Inspector and place it.",
                ),
            };
            p.spawn((
                UiBlocker,
                Interaction::default(),
                Node {
                    padding: UiRect::axes(px(14), px(10)),
                    column_gap: px(10),
                    align_items: AlignItems::Center,
                    border: UiRect::all(px(1)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(t.panel.with_alpha(0.92)),
                BorderColor::all(t.border),
            ))
            .with_children(|p| {
                p.spawn(glyph(cx, "✦", 16., t.accent_text));
                p.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2),
                    ..default()
                })
                .with_children(|p| {
                    p.spawn(text(cx, title, size::MD, t.text, Weight::Semibold));
                    p.spawn(text(cx, body, size::SM, t.text_dim, Weight::Regular));
                });
                if kind == DocKind::Body {
                    button(
                        p,
                        cx,
                        Btn::new("Add root bone", Action::AddBone { child: false }),
                    );
                    button(
                        p,
                        cx,
                        Btn::new("Humanoid template", Action::DefaultSkeleton).primary(),
                    );
                }
            });
        });
    }
}

fn status_bar(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    let e = cx.e;
    let l = &e.library;
    let color = match e.notice.kind {
        NoticeKind::Info => t.text_dim,
        NoticeKind::Success => t.success,
        NoticeKind::Error => t.danger,
    };
    p.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            height: px(metric::STATUS_BAR),
            flex_shrink: 0.,
            padding: UiRect::horizontal(px(10)),
            align_items: AlignItems::Center,
            column_gap: px(8),
            border: UiRect::top(px(1)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(t.panel_header),
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        if !e.notice.text.is_empty() {
            p.spawn(glyph(
                cx,
                match e.notice.kind {
                    NoticeKind::Error => "⚠",
                    NoticeKind::Success => "✓",
                    NoticeKind::Info => "●",
                },
                10.,
                color,
            ));
            p.spawn(text(
                cx,
                ellipsize(&e.notice.text, 110),
                size::SM,
                color,
                Weight::Regular,
            ));
        }
        spacer(p);
        p.spawn(text(
            cx,
            format!(
                "{} bodies · {} props · {} characters · {} tiles · {} worlds",
                l.bodies.len(),
                l.props.len(),
                l.characters.len(),
                l.tiles.len(),
                l.worlds.len()
            ),
            size::SM,
            t.text_muted,
            Weight::Regular,
        ));
        vdivider(p, cx, 12.);
        let dirty = e.dirty();
        p.spawn(glyph(
            cx,
            "●",
            8.,
            if dirty { t.warning } else { t.success },
        ));
        p.spawn(text(
            cx,
            if dirty {
                "Unsaved changes"
            } else {
                "All changes saved"
            },
            size::SM,
            t.text_dim,
            Weight::Regular,
        ));
    });
}
