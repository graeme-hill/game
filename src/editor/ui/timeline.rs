//! The animation timeline docked under the viewport: transport, ruler,
//! key tracks and the playhead.
use super::theme::{metric, size};
use super::widgets::*;
use super::*;

pub fn timeline(p: Kids, cx: &Ctx) {
    let t = &cx.t;
    let e = cx.e;
    let Some(clip) = e.animation_clip() else {
        return;
    };
    let duration = clip.duration.max(0.001);
    p.spawn((
        UiBlocker,
        Interaction::default(),
        Node {
            min_height: px(metric::TIMELINE),
            flex_shrink: 0.,
            flex_direction: FlexDirection::Column,
            border: UiRect::top(px(1)),
            ..default()
        },
        BackgroundColor(t.panel),
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        // Transport bar; wraps onto a second row in narrow windows.
        p.spawn((
            Node {
                min_height: px(38),
                padding: UiRect::axes(px(10), px(5)),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: px(4),
                row_gap: px(4),
                border: UiRect::bottom(px(1)),
                ..default()
            },
            BorderColor::all(t.divider),
        ))
        .with_children(|p| {
            // Controls are grouped so the bar wraps between groups.
            let group = |p: Kids, body: &dyn Fn(Kids)| {
                p.spawn(Node {
                    column_gap: px(4),
                    align_items: AlignItems::Center,
                    flex_shrink: 0.,
                    ..default()
                })
                .with_children(|p| body(p));
            };
            group(p, &|p| {
                icon_button(p, cx, "⇤", Action::Anim(AnimOp::Start), "Go to start");
                icon_button(
                    p,
                    cx,
                    "◂",
                    Action::Anim(AnimOp::PrevKey),
                    "Previous key (,)",
                );
                button(
                    p,
                    cx,
                    Btn::new("", Action::Anim(AnimOp::Play))
                        .icon(if e.anim.playing { "❚❚" } else { "▶" })
                        .primary()
                        .tip("Play / pause (Space)"),
                );
                icon_button(p, cx, "▸", Action::Anim(AnimOp::NextKey), "Next key (.)");
                icon_button(p, cx, "⇥", Action::Anim(AnimOp::End), "Go to end");
                p.spawn((
                    Node {
                        width: px(58),
                        justify_content: JustifyContent::End,
                        ..default()
                    },
                    children![(
                        text(
                            cx,
                            format!("{:.2}s", e.anim.time),
                            size::MD,
                            t.text,
                            Weight::Mono
                        ),
                        LiveLabel::Time
                    )],
                ));
                p.spawn(text(
                    cx,
                    format!("/ {:.2}s", clip.duration),
                    size::MD,
                    t.text_muted,
                    Weight::Mono,
                ));
            });
            group(p, &|p| {
                vdivider(p, cx, 18.);
                checkbox(p, cx, "Loop", clip.looping, Action::Anim(AnimOp::Loop));
                vdivider(p, cx, 18.);
                let has_bone = matches!(e.sel(), Sel::Bone(_));
                button(
                    p,
                    cx,
                    Btn::new("Key", Action::Anim(AnimOp::Key))
                        .icon("◆")
                        .enabled(has_bone)
                        .tip("Record the selected bone's pose at the playhead (K)"),
                );
                button(
                    p,
                    cx,
                    Btn::new("", Action::Anim(AnimOp::DeleteKey))
                        .icon("✕")
                        .ghost()
                        .enabled(has_bone)
                        .tip("Delete the selected bone's key at the playhead"),
                );
            });
            spacer(p);
            group(p, &|p| {
                p.spawn(text(cx, "Blend", size::SM, t.text_muted, Weight::Regular));
                p.spawn(Node {
                    width: px(120),
                    ..default()
                })
                .with_children(|p| {
                    let target = e
                        .anim
                        .blend_target
                        .and_then(|i| e.active_character()?.animations.get(i))
                        .map_or("None".to_string(), |c| c.name.clone());
                    dropdown(p, cx, Menu::BlendTarget, &target, None);
                });
                if e.anim.blend_target.is_some() {
                    p.spawn(Node {
                        width: px(60),
                        ..default()
                    })
                    .with_children(|p| number(p, cx, Field::Blend, None, true));
                }
                icon_button(
                    p,
                    cx,
                    "✕",
                    Action::Anim(AnimOp::Close),
                    "Close the timeline",
                );
            });
        });
        // Track names on the left; ruler, keys and playhead on the right.
        let selected = match e.sel() {
            Sel::Bone(id) => Some(id),
            _ => None,
        };
        let bone_name = selected
            .and_then(|id| e.preview_body()?.bones.iter().find(|b| b.id == id))
            .map(|b| b.name.clone());
        let mut all: Vec<f32> = clip
            .tracks
            .iter()
            .flat_map(|tr| tr.keys.iter().map(|k| k.time))
            .collect();
        all.sort_by(f32::total_cmp);
        all.dedup_by(|a, b| (*a - *b).abs() < 0.0005);
        let mine: Vec<f32> = selected
            .and_then(|id| clip.tracks.iter().find(|tr| tr.bone == id))
            .map(|tr| tr.keys.iter().map(|k| k.time).collect())
            .unwrap_or_default();
        let tracks = [
            ("All bones".to_string(), all, t.text_dim),
            (
                bone_name.unwrap_or_else(|| "No bone selected".into()),
                mine,
                t.warning,
            ),
        ];
        p.spawn(Node {
            flex_grow: 1.,
            padding: UiRect {
                right: px(18),
                top: px(4),
                bottom: px(10),
                ..default()
            },
            ..default()
        })
        .with_children(|p| {
            p.spawn(Node {
                width: px(130),
                flex_shrink: 0.,
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|p| {
                p.spawn(Node {
                    height: px(20),
                    padding: UiRect::left(px(12)),
                    column_gap: px(6),
                    align_items: AlignItems::Center,
                    overflow: Overflow::clip(),
                    ..default()
                })
                .with_children(|p| {
                    p.spawn(glyph(cx, "▶", 9., t.kind(DocKind::Character)));
                    p.spawn(text(
                        cx,
                        ellipsize(&clip.name, 15),
                        size::SM,
                        t.text,
                        Weight::Semibold,
                    ));
                });
                for (name, _, color) in &tracks {
                    p.spawn((
                        Node {
                            height: px(26),
                            padding: UiRect::horizontal(px(12)),
                            column_gap: px(6),
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(px(1)),
                            overflow: Overflow::clip(),
                            ..default()
                        },
                        BorderColor::all(t.divider),
                    ))
                    .with_children(|p| {
                        p.spawn(glyph(cx, "◆", 8., *color));
                        p.spawn(text(
                            cx,
                            ellipsize(name, 16),
                            size::SM,
                            t.text_dim,
                            Weight::Regular,
                        ));
                    });
                }
            });
            let mut c = control(
                Action::Anim(AnimOp::Start),
                "Timeline",
                ControlKind::Timeline,
            );
            c.tooltip = String::new();
            p.spawn((
                Button,
                c,
                Node {
                    flex_grow: 1.,
                    position_type: PositionType::Relative,
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(Color::NONE),
            ))
            .with_children(|p| {
                // Ruler ticks; labels thin out for long clips.
                let step = if duration > 4. { 0.5 } else { 0.1 };
                let count = (duration / step).round() as usize;
                let label_every = if count > 20 { 5 } else { 2 };
                p.spawn(Node {
                    height: px(20),
                    position_type: PositionType::Relative,
                    border: UiRect::bottom(px(1)),
                    ..default()
                })
                .insert(BorderColor::all(t.border))
                .with_children(|p| {
                    for i in 0..=count {
                        let x = (i as f32 * step / duration * 100.).min(100.);
                        let major = i % label_every == 0;
                        p.spawn((
                            Node {
                                position_type: PositionType::Absolute,
                                left: percent(x),
                                bottom: px(0),
                                width: px(1),
                                height: px(if major { 8 } else { 4 }),
                                ..default()
                            },
                            BackgroundColor(if major { t.text_muted } else { t.border }),
                        ));
                        // The last label would run past the edge.
                        if major && x < 96. {
                            p.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: percent(x),
                                    top: px(0),
                                    margin: UiRect::left(px(3)),
                                    ..default()
                                },
                                children![text(
                                    cx,
                                    format!("{:.1}", i as f32 * step),
                                    size::XS,
                                    t.text_muted,
                                    Weight::Mono
                                )],
                            ));
                        }
                    }
                });
                for (track, (_, keys, color)) in tracks.into_iter().enumerate() {
                    let bone = if track == 0 { None } else { selected };
                    p.spawn((
                        Node {
                            height: px(26),
                            position_type: PositionType::Relative,
                            align_items: AlignItems::Center,
                            border: UiRect::bottom(px(1)),
                            ..default()
                        },
                        BorderColor::all(t.divider),
                    ))
                    .with_children(|p| {
                        for k in keys {
                            let mut c = control(
                                Action::SetValue(Field::Time, k),
                                format!("Key {k:.2}s"),
                                ControlKind::Key { bone, time: k },
                            );
                            c.tooltip = format!(
                                "{:.2}s — drag to retime{}, click to jump",
                                k,
                                if bone.is_none() {
                                    " every bone's key"
                                } else {
                                    ""
                                }
                            );
                            p.spawn((
                                Button,
                                c,
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: percent(k / duration * 100.),
                                    width: px(14),
                                    height: px(20),
                                    margin: UiRect::left(px(-7)),
                                    justify_content: JustifyContent::Center,
                                    align_items: AlignItems::Center,
                                    border_radius: BorderRadius::all(px(3)),
                                    ..default()
                                },
                                BackgroundColor(Color::NONE),
                                HoverStyle {
                                    idle: Color::NONE,
                                    hover: t.hover,
                                    press: t.press,
                                },
                                children![glyph(cx, "◆", 11., color)],
                            ));
                        }
                    });
                }
                // Playhead.
                p.spawn((
                    LiveLabel::Playhead,
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(e.anim.time / duration * 100.),
                        top: px(0),
                        bottom: px(0),
                        width: px(2),
                        margin: UiRect::left(px(-1)),
                        ..default()
                    },
                    BackgroundColor(t.accent),
                ))
                .with_children(|p| {
                    p.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            top: px(-2),
                            left: px(-5),
                            width: px(12),
                            height: px(10),
                            border_radius: BorderRadius::bottom(px(4)),
                            ..default()
                        },
                        BackgroundColor(t.accent),
                    ));
                });
            });
        });
    });
}
