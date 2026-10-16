//! The socket rules document: a symmetric compatibility matrix.
use super::theme::size;
use super::widgets::*;
use super::*;

/// Socket compatibility as a symmetric matrix.
pub fn rules_matrix(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    let e = cx.e;
    let kinds = actions::socket_kinds(&e.library);
    let rules = &e.library.socket_rules;
    // Fit the matrix to the center area: window minus side panels and padding.
    let available = cx.center_width() - 48. - 24. - 24.;
    let name_width = if available < 520. { 112. } else { 140. };
    // Columns narrow to fit; rows stay tall enough to read. If it still does
    // not fit, the matrix scrolls sideways (Shift+wheel).
    let cell = ((available - name_width - 24.) / kinds.len().max(1) as f32).clamp(12., 24.);
    let row = cell.max(18.);
    scroll(p, cx, "rules", UiRect::all(px(24)), 14., |p| {
        p.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: px(4),
                flex_shrink: 0.,
                ..default()
            })
            .with_children(|p| {
                p.spawn(text(cx, "Socket compatibility", size::XL, t.text, Weight::Semibold));
                p.spawn(wrapped(
                    cx,
                    "Two sockets may connect when their types are paired here and their profiles match. Pairs are symmetric.",
                    size::MD,
                    t.text_dim,
                ));
            });
        let offset = cx.scroll.0.get("rules-matrix:x").copied().unwrap_or(0.);
        p.spawn((
            ScrollArea("rules-matrix".into()),
            ScrollPosition(Vec2::new(offset, 0.)),
            Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(px(12)),
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(8)),
                flex_shrink: 0.,
                align_self: AlignSelf::FlexStart,
                max_width: percent(100),
                overflow: Overflow::scroll_x(),
                ..default()
            },
            BackgroundColor(t.panel),
            BorderColor::all(t.border),
        ))
        .with_children(|p| {
            // Column numbers.
            p.spawn(Node {
                height: px(cell),
                ..default()
            })
            .with_children(|p| {
                p.spawn(Node {
                    width: px(name_width + 24.),
                    ..default()
                });
                for (j, _) in kinds.iter().enumerate() {
                    p.spawn((
                        Node {
                            width: px(cell),
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        // Narrow cells only label every fifth column.
                        children![text(
                            cx,
                            if cell >= 18. || (j + 1) % 5 == 0 || j == 0 {
                                (j + 1).to_string()
                            } else {
                                String::new()
                            },
                            size::XS,
                            t.text_muted,
                            Weight::Mono
                        )],
                    ));
                }
            });
            for (i, a) in kinds.iter().enumerate() {
                let row_focus = e.rule_focus.as_ref().is_some_and(|(x, y)| x == a || y == a);
                p.spawn(Node {
                    height: px(row),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|p| {
                    p.spawn((
                        Node {
                            width: px(24),
                            ..default()
                        },
                        children![text(
                            cx,
                            (i + 1).to_string(),
                            size::XS,
                            t.text_muted,
                            Weight::Mono
                        )],
                    ));
                    p.spawn((
                        Node {
                            width: px(name_width),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        children![text(
                            cx,
                            ellipsize(a, 20),
                            size::SM,
                            if row_focus { t.accent_text } else { t.text },
                            Weight::Regular
                        )],
                    ));
                    for (j, b) in kinds.iter().enumerate() {
                        let on = rules.allows(a, b);
                        let focus = e
                            .rule_focus
                            .as_ref()
                            .is_some_and(|(x, y)| (x == a && y == b) || (x == b && y == a));
                        let diagonal = i == j;
                        let idle = if on {
                            t.accent
                        } else if diagonal {
                            t.raised
                        } else if (i + j) % 2 == 0 {
                            t.field
                        } else {
                            t.panel_header
                        };
                        let mut c = control(
                            Action::ToggleRule(a.clone(), b.clone()),
                            format!("{a} ↔ {b}"),
                            ControlKind::Button,
                        );
                        c.tooltip = format!(
                            "{a} ↔ {b}: {}",
                            if on {
                                "allowed — click to forbid"
                            } else {
                                "not allowed — click to allow"
                            }
                        );
                        p.spawn((
                            Button,
                            c,
                            Node {
                                width: px(cell - 2.),
                                height: px(row - 2.),
                                margin: UiRect::all(px(1)),
                                justify_content: JustifyContent::Center,
                                align_items: AlignItems::Center,
                                border: UiRect::all(px(if focus { 2 } else { 0 })),
                                border_radius: BorderRadius::all(px(3)),
                                ..default()
                            },
                            BackgroundColor(idle),
                            BorderColor::all(Color::WHITE),
                            HoverStyle {
                                idle,
                                hover: if on { t.accent_hover } else { t.hover },
                                press: t.press,
                            },
                        ))
                        .with_children(|p| {
                            if on {
                                p.spawn(glyph(cx, "✓", (cell * 0.5).min(11.), Color::WHITE));
                            }
                        });
                    }
                });
            }
        });
        p.spawn(Node {
            column_gap: px(8),
            align_items: AlignItems::Center,
            flex_shrink: 0.,
            max_width: px(420),
            ..default()
        })
        .with_children(|p| {
            text_input(p, cx, TextTarget::NewRuleType, "", "New socket type…", true);
            button(
                p,
                cx,
                Btn::new("Add type", Action::Edit(TextTarget::NewRuleType)).icon("+"),
            );
        });
    });
}
