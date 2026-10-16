//! Reusable widgets: buttons, property rows, numeric/vector fields, text
//! inputs, dropdowns, segmented controls, sections and scroll panes.
use super::theme::{metric, size};
use super::*;

pub type Kids<'a, 'b> = &'a mut ChildSpawnerCommands<'b>;

#[derive(Clone, Copy, PartialEq)]
pub enum Weight {
    Regular,
    Medium,
    Semibold,
    Mono,
}

pub fn font(cx: &Ctx, weight: Weight) -> Handle<Font> {
    match weight {
        Weight::Regular => cx.fonts.regular.clone(),
        Weight::Medium => cx.fonts.medium.clone(),
        Weight::Semibold => cx.fonts.semibold.clone(),
        Weight::Mono => cx.fonts.mono.clone(),
    }
}

pub fn text(cx: &Ctx, s: impl Into<String>, px: f32, color: Color, weight: Weight) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: FontSource::Handle(font(cx, weight)),
            font_size: FontSize::Px(px),
            ..default()
        },
        TextColor(color),
        TextLayout::no_wrap(),
    )
}

pub fn wrapped(cx: &Ctx, s: impl Into<String>, px: f32, color: Color) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font: FontSource::Handle(cx.fonts.regular.clone()),
            font_size: FontSize::Px(px),
            ..default()
        },
        TextColor(color),
    )
}

pub fn glyph(cx: &Ctx, g: &str, px: f32, color: Color) -> impl Bundle {
    (
        Text::new(g),
        TextFont {
            font: FontSource::Handle(cx.fonts.icons.clone()),
            font_size: FontSize::Px(px),
            ..default()
        },
        TextColor(color),
        TextLayout::no_wrap(),
    )
}

/// Shorten long names so fixed-width rows stay tidy.
pub fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

pub fn control(action: Action, label: impl Into<String>, kind: ControlKind) -> Control {
    Control {
        action,
        label: label.into(),
        kind,
        context: None,
        tooltip: String::new(),
        middle: None,
    }
}

pub fn spacer(p: Kids) {
    p.spawn(Node {
        flex_grow: 1.,
        ..default()
    });
}

pub fn divider(p: Kids, cx: &Ctx) {
    p.spawn((
        Node {
            height: px(1),
            width: percent(100),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(cx.t.border),
    ));
}

pub fn vdivider(p: Kids, cx: &Ctx, height: f32) {
    p.spawn((
        Node {
            width: px(1),
            height: px(height),
            margin: UiRect::horizontal(px(4)),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(cx.t.border),
    ));
}

#[derive(Clone, Copy, PartialEq)]
pub enum Tone {
    Primary,
    Normal,
    Ghost,
    Danger,
    Success,
}

pub struct Btn {
    pub label: String,
    pub icon: Option<&'static str>,
    pub icon_color: Option<Color>,
    pub action: Action,
    pub tone: Tone,
    pub active: bool,
    pub enabled: bool,
    pub width: Option<f32>,
    pub grow: bool,
    pub tooltip: String,
    pub height: f32,
}

impl Btn {
    pub fn new(label: impl Into<String>, action: Action) -> Self {
        Self {
            label: label.into(),
            icon: None,
            icon_color: None,
            action,
            tone: Tone::Normal,
            active: false,
            enabled: true,
            width: None,
            grow: false,
            tooltip: String::new(),
            height: 26.,
        }
    }
    pub fn icon(mut self, glyph: &'static str) -> Self {
        self.icon = Some(glyph);
        self
    }
    pub fn icon_color(mut self, color: Color) -> Self {
        self.icon_color = Some(color);
        self
    }
    pub fn tone(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
    pub fn primary(self) -> Self {
        self.tone(Tone::Primary)
    }
    pub fn ghost(self) -> Self {
        self.tone(Tone::Ghost)
    }
    pub fn danger(self) -> Self {
        self.tone(Tone::Danger)
    }
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn grow(mut self) -> Self {
        self.grow = true;
        self
    }
    pub fn tip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = tooltip.into();
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }
}

pub fn button(p: Kids, cx: &Ctx, b: Btn) {
    let t = &cx.t;
    let (idle, hover, press, fg, border) = match (b.tone, b.active) {
        (_, true) => (
            t.accent_dim,
            t.accent_dim,
            t.press,
            t.accent_text,
            t.accent.with_alpha(0.6),
        ),
        (Tone::Primary, _) => (t.accent, t.accent_hover, t.accent, Color::WHITE, t.accent),
        (Tone::Normal, _) => (t.raised, t.hover, t.press, t.text, t.border),
        (Tone::Ghost, _) => (Color::NONE, t.hover, t.press, t.text_dim, Color::NONE),
        (Tone::Danger, _) => (t.raised, t.danger_dim, t.press, t.danger, t.border),
        (Tone::Success, _) => (
            t.success.with_alpha(0.85),
            t.success,
            t.success,
            Color::WHITE,
            t.success,
        ),
    };
    // Disabled buttons keep their shape but drop their emphasis.
    let (idle, border, fg) = match (b.enabled, b.tone) {
        (true, _) => (idle, border, fg),
        (false, Tone::Ghost) => (idle, border, t.text_muted.with_alpha(0.6)),
        (false, _) => (
            t.raised.with_alpha(0.5),
            t.border.with_alpha(0.5),
            t.text_muted,
        ),
    };
    let square = b.label.is_empty();
    let mut node = Node {
        height: px(b.height),
        min_width: px(if square { b.height } else { 0. }),
        padding: UiRect::horizontal(px(if square { 0. } else { 10. })),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        column_gap: px(6),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(metric::RADIUS)),
        flex_shrink: 0.,
        ..default()
    };
    if let Some(width) = b.width {
        node.width = px(width);
    }
    if b.grow {
        node.flex_grow = 1.;
        node.flex_basis = px(0);
    }
    let mut c = control(b.action, b.label.clone(), ControlKind::Button);
    c.tooltip = b.tooltip;
    let mut entity = p.spawn((node, BackgroundColor(idle), BorderColor::all(border)));
    if b.enabled {
        entity.insert((Button, c, HoverStyle { idle, hover, press }));
    } else {
        entity.insert((Interaction::default(), UiBlocker));
    }
    let icon_color = b.icon_color.filter(|_| b.enabled).unwrap_or(fg);
    entity.with_children(|p| {
        if let Some(g) = b.icon {
            p.spawn(glyph(cx, g, if square { 14. } else { 12.5 }, icon_color));
        }
        if !square {
            p.spawn(text(cx, b.label, size::MD, fg, Weight::Medium));
        }
    });
}

/// A compact square icon button.
pub fn icon_button(p: Kids, cx: &Ctx, g: &'static str, action: Action, tip: &str) {
    button(
        p,
        cx,
        Btn::new("", action).icon(g).ghost().tip(tip).height(26.),
    );
}

/// Mutually exclusive options sharing one frame.
pub fn segmented(p: Kids, cx: &Ctx, items: Vec<(String, Action, bool, String)>, grow: bool) {
    let t = &cx.t;
    p.spawn((
        Node {
            height: px(26),
            padding: UiRect::all(px(2)),
            column_gap: px(2),
            border_radius: BorderRadius::all(px(metric::RADIUS + 1.)),
            flex_grow: if grow { 1. } else { 0. },
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(t.field),
    ))
    .with_children(|p| {
        for (label, action, active, tip) in items {
            let (idle, fg) = if active {
                (t.raised, t.text)
            } else {
                (Color::NONE, t.text_dim)
            };
            let glyph_only =
                label.chars().count() == 1 && !label.chars().all(char::is_alphanumeric);
            let mut c = control(action, label.clone(), ControlKind::Button);
            c.tooltip = tip;
            p.spawn((
                Button,
                c,
                Node {
                    flex_grow: if grow { 1. } else { 0. },
                    min_width: px(24),
                    padding: UiRect::horizontal(px(8)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(metric::RADIUS - 1.)),
                    ..default()
                },
                BackgroundColor(idle),
                HoverStyle {
                    idle,
                    hover: if active { t.raised } else { t.hover },
                    press: t.press,
                },
                BoxShadow::new(
                    if active {
                        t.shadow.with_alpha(0.35)
                    } else {
                        Color::NONE
                    },
                    px(0),
                    px(1),
                    px(0),
                    px(2),
                ),
            ))
            .with_children(|p| {
                if glyph_only {
                    p.spawn(glyph(cx, &label, 13., fg));
                } else {
                    p.spawn(text(cx, label, size::SM, fg, Weight::Medium));
                }
            });
        }
    });
}

/// A collapsible inspector section with an uppercase header.
pub fn section(p: Kids, cx: &Ctx, key: &str, title: &str, body: impl FnOnce(Kids)) {
    let t = &cx.t;
    let folded = cx.e.collapsed.contains(&format!("section:{key}"));
    p.spawn((
        Node {
            flex_direction: FlexDirection::Column,
            flex_shrink: 0.,
            border: UiRect::bottom(px(1)),
            ..default()
        },
        BorderColor::all(t.divider),
    ))
    .with_children(|p| {
        p.spawn((
            Button,
            control(
                Action::ToggleFolder(format!("section:{key}")),
                title.to_string(),
                ControlKind::Button,
            ),
            Node {
                height: px(30),
                padding: UiRect::horizontal(px(12)),
                align_items: AlignItems::Center,
                column_gap: px(6),
                ..default()
            },
            BackgroundColor(Color::NONE),
            HoverStyle {
                idle: Color::NONE,
                hover: t.hover.with_alpha(0.5),
                press: t.press,
            },
        ))
        .with_children(|p| {
            p.spawn(glyph(cx, if folded { "▸" } else { "▾" }, 10., t.text_muted));
            p.spawn(text(
                cx,
                title.to_uppercase(),
                size::XS,
                t.text_dim,
                Weight::Semibold,
            ));
        });
        if !folded {
            p.spawn(Node {
                flex_direction: FlexDirection::Column,
                padding: UiRect {
                    left: px(12),
                    right: px(12),
                    top: px(2),
                    bottom: px(12),
                },
                row_gap: px(6),
                ..default()
            })
            .with_children(body);
        }
    });
}

/// "Label  [content]" with a fixed label column.
pub fn row(p: Kids, cx: &Ctx, name: &str, body: impl FnOnce(Kids)) {
    p.spawn(Node {
        min_height: px(metric::FIELD),
        align_items: AlignItems::Center,
        column_gap: px(4),
        flex_shrink: 0.,
        ..default()
    })
    .with_children(|p| {
        p.spawn(Node {
            width: px(metric::LABEL),
            flex_shrink: 0.,
            overflow: Overflow::clip(),
            ..default()
        })
        .with_children(|p| {
            p.spawn(text(cx, name, size::SM, cx.t.text_dim, Weight::Regular));
        });
        p.spawn(Node {
            flex_grow: 1.,
            flex_basis: px(0),
            column_gap: px(4),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(body);
    });
}

/// A plain row of widgets.
pub fn hstack(p: Kids, gap: f32, body: impl FnOnce(Kids)) {
    p.spawn(Node {
        column_gap: px(gap),
        align_items: AlignItems::Center,
        flex_shrink: 0.,
        flex_wrap: FlexWrap::Wrap,
        row_gap: px(gap),
        ..default()
    })
    .with_children(body);
}

fn editing<'a>(cx: &'a Ctx, target: &TextTarget) -> Option<&'a TextEdit> {
    cx.e.editing.as_ref().filter(|edit| &edit.target == target)
}

/// The text of an active input: selection highlight when fresh, else a caret.
fn edit_text(p: Kids, cx: &Ctx, edit: &TextEdit, weight: Weight, px_size: f32) {
    let t = &cx.t;
    if edit.fresh && !edit.buffer.is_empty() {
        p.spawn((
            Node {
                padding: UiRect::horizontal(px(1)),
                border_radius: BorderRadius::all(px(2)),
                ..default()
            },
            BackgroundColor(t.accent.with_alpha(0.45)),
            children![text(cx, edit.buffer.clone(), px_size, t.text, weight)],
        ));
        return;
    }
    let caret = edit.caret.min(edit.buffer.chars().count());
    let before: String = edit.buffer.chars().take(caret).collect();
    let after: String = edit.buffer.chars().skip(caret).collect();
    if !before.is_empty() {
        p.spawn(text(cx, before, px_size, t.text, weight));
    }
    p.spawn((
        Node {
            width: px(1.5),
            height: px(px_size + 2.),
            margin: UiRect::horizontal(px(0.5)),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(t.accent_text),
    ));
    if !after.is_empty() {
        p.spawn(text(cx, after, px_size, t.text, weight));
    }
}

fn field_frame(
    cx: &Ctx,
    active: bool,
    grow: bool,
) -> (Node, BackgroundColor, BorderColor, HoverStyle) {
    let t = &cx.t;
    (
        Node {
            height: px(metric::FIELD),
            min_width: px(0),
            flex_grow: if grow { 1. } else { 0. },
            flex_basis: if grow { px(0) } else { Val::Auto },
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(t.field),
        BorderColor::all(if active { t.accent } else { t.border }),
        HoverStyle {
            idle: t.field,
            hover: t.field_hover,
            press: t.field_hover,
        },
    )
}

/// A numeric field: drag horizontally to scrub, click to type.
pub fn number(p: Kids, cx: &Ctx, field: Field, tag: Option<(&str, Color)>, grow: bool) {
    let t = &cx.t;
    let target = TextTarget::Value(field);
    let edit = editing(cx, &target);
    let value = fields::get(cx.e, field);
    let shown = value.map_or("—".to_string(), |v| field.format(v));
    let mut c = control(
        Action::Edit(target),
        shown.clone(),
        ControlKind::Field(field),
    );
    c.tooltip = String::new();
    p.spawn((Button, c, field_frame(cx, edit.is_some(), grow)))
        .with_children(|p| {
            if let Some((letter, color)) = tag {
                p.spawn((
                    Node {
                        width: px(14),
                        height: percent(100),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        flex_shrink: 0.,
                        ..default()
                    },
                    BackgroundColor(color.with_alpha(0.16)),
                    children![text(cx, letter, size::XS, color, Weight::Semibold)],
                ));
            }
            p.spawn(Node {
                padding: UiRect::horizontal(px(6)),
                align_items: AlignItems::Center,
                flex_grow: 1.,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|p| match edit {
                Some(edit) => edit_text(p, cx, edit, Weight::Mono, size::SM),
                None => {
                    p.spawn(text(
                        cx,
                        shown,
                        size::SM,
                        if value.is_some() {
                            t.text
                        } else {
                            t.text_muted
                        },
                        Weight::Mono,
                    ));
                }
            });
        });
}

/// Three axis fields for a vector property.
pub fn vector(p: Kids, cx: &Ctx, name: &str, fields: [Field; 3]) {
    row(p, cx, name, |p| {
        for (axis, field) in fields.into_iter().enumerate() {
            number(
                p,
                cx,
                field,
                Some((["X", "Y", "Z"][axis], cx.t.axis(axis))),
                true,
            );
        }
    });
}

/// A single-line text input showing `display` until clicked.
pub fn text_input(
    p: Kids,
    cx: &Ctx,
    target: TextTarget,
    display: &str,
    placeholder: &str,
    grow: bool,
) {
    let t = &cx.t;
    let edit = editing(cx, &target);
    let c = control(
        Action::Edit(target.clone()),
        display.to_string(),
        ControlKind::Text(target),
    );
    p.spawn((Button, c, field_frame(cx, edit.is_some(), grow)))
        .with_children(|p| {
            p.spawn(Node {
                padding: UiRect::horizontal(px(8)),
                align_items: AlignItems::Center,
                flex_grow: 1.,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|p| match edit {
                Some(edit) => edit_text(p, cx, edit, Weight::Regular, size::MD),
                None if display.is_empty() => {
                    p.spawn(text(
                        cx,
                        placeholder,
                        size::MD,
                        t.text_muted,
                        Weight::Regular,
                    ));
                }
                None => {
                    p.spawn(text(cx, display, size::MD, t.text, Weight::Regular));
                }
            });
        });
}

/// A document title: reads as a heading, edits in place when clicked.
pub fn title_input(p: Kids, cx: &Ctx, target: TextTarget, display: &str) {
    let t = &cx.t;
    let edit = editing(cx, &target);
    let c = control(
        Action::Edit(target.clone()),
        display.to_string(),
        ControlKind::Text(target),
    );
    let idle = if edit.is_some() { t.field } else { Color::NONE };
    p.spawn((
        Button,
        c,
        Node {
            height: px(26),
            padding: UiRect::horizontal(px(5)),
            margin: UiRect::left(px(-6)),
            align_items: AlignItems::Center,
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(idle),
        BorderColor::all(if edit.is_some() {
            t.accent
        } else {
            Color::NONE
        }),
        HoverStyle {
            idle,
            hover: if edit.is_some() {
                idle
            } else {
                t.hover.with_alpha(0.6)
            },
            press: idle,
        },
    ))
    .with_children(|p| match edit {
        Some(edit) => edit_text(p, cx, edit, Weight::Semibold, size::LG),
        None => {
            p.spawn(text(
                cx,
                ellipsize(display, 26),
                size::LG,
                t.text,
                Weight::Semibold,
            ));
        }
    });
}

/// A field-styled button that opens a menu of choices.
pub fn dropdown(p: Kids, cx: &Ctx, menu: Menu, current: &str, icon: Option<(&'static str, Color)>) {
    let t = &cx.t;
    let open = cx.e.menu.as_ref().is_some_and(|m| m.menu == menu);
    let c = control(
        Action::OpenMenu(menu),
        current.to_string(),
        ControlKind::Button,
    );
    p.spawn((Button, c, field_frame(cx, open, true)))
        .with_children(|p| {
            p.spawn(Node {
                padding: UiRect::left(px(8)),
                column_gap: px(6),
                align_items: AlignItems::Center,
                flex_grow: 1.,
                overflow: Overflow::clip(),
                ..default()
            })
            .with_children(|p| {
                if let Some((g, color)) = icon {
                    p.spawn(glyph(cx, g, 11., color));
                }
                p.spawn(text(
                    cx,
                    ellipsize(current, 26),
                    size::MD,
                    if current.is_empty() {
                        t.text_muted
                    } else {
                        t.text
                    },
                    Weight::Regular,
                ));
            });
            p.spawn((
                Node {
                    width: px(22),
                    justify_content: JustifyContent::Center,
                    flex_shrink: 0.,
                    ..default()
                },
                children![glyph(cx, "▾", 11., t.text_dim)],
            ));
        });
}

/// A checkbox with a label.
pub fn checkbox(p: Kids, cx: &Ctx, name: &str, on: bool, action: Action) {
    let t = &cx.t;
    p.spawn((
        Button,
        control(action, name.to_string(), ControlKind::Button),
        Node {
            height: px(metric::FIELD),
            align_items: AlignItems::Center,
            column_gap: px(8),
            padding: UiRect::right(px(6)),
            border_radius: BorderRadius::all(px(4)),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(Color::NONE),
        HoverStyle {
            idle: Color::NONE,
            hover: t.hover.with_alpha(0.4),
            press: t.press,
        },
    ))
    .with_children(|p| {
        p.spawn((
            Node {
                width: px(16),
                height: px(16),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(1)),
                border_radius: BorderRadius::all(px(4)),
                ..default()
            },
            BackgroundColor(if on { t.accent } else { t.field }),
            BorderColor::all(if on { t.accent } else { t.border }),
        ))
        .with_children(|p| {
            if on {
                p.spawn(glyph(cx, "✓", 11., Color::WHITE));
            }
        });
        p.spawn(text(cx, name, size::MD, t.text, Weight::Regular));
    });
}

pub fn swatch(p: Kids, cx: &Ctx, color: [f32; 3], size: f32) {
    p.spawn((
        Node {
            width: px(size),
            height: px(size),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(Color::srgb(color[0], color[1], color[2])),
        BorderColor::all(cx.t.border),
    ));
}

/// An HSV picker: a saturation/value square over the current hue, and a hue
/// strip. Drag either to choose; the result goes through the RGB fields.
pub fn color_picker(p: Kids, cx: &Ctx, current: [f32; 3]) {
    let t = &cx.t;
    let (hue, saturation, value) = fields::rgb_to_hsv(current);
    let hue = if saturation < 0.001 || value < 0.001 {
        cx.e.hue
    } else {
        hue
    };
    let pure = fields::hsv_to_rgb(hue, 1., 1.);
    let srgb = |c: [f32; 3]| Color::srgb(c[0], c[1], c[2]);
    let marker = |p: Kids, left: f32, top: Option<f32>| {
        p.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(left * 100.),
                top: top.map_or(px(-2), |y| percent(y * 100.)),
                width: px(if top.is_some() { 12 } else { 6 }),
                height: if top.is_some() { px(12) } else { px(16) },
                margin: UiRect {
                    left: px(if top.is_some() { -6. } else { -3. }),
                    top: px(if top.is_some() { -6. } else { 0. }),
                    ..default()
                },
                border: UiRect::all(px(2)),
                border_radius: BorderRadius::all(px(if top.is_some() { 6. } else { 2. })),
                ..default()
            },
            BorderColor::all(Color::WHITE),
            BoxShadow::new(Color::BLACK.with_alpha(0.6), px(0), px(0), px(0), px(2)),
        ));
    };
    p.spawn((
        Button,
        control(Action::CloseMenu, "Color square", ControlKind::Picker(true)),
        Node {
            height: px(110),
            border_radius: BorderRadius::all(px(4)),
            flex_shrink: 0.,
            position_type: PositionType::Relative,
            ..default()
        },
        BackgroundGradient(vec![
            LinearGradient::to_right(vec![Color::WHITE.into(), srgb(pure).into()])
                .in_color_space(InterpolationColorSpace::Srgba)
                .into(),
            LinearGradient::to_bottom(vec![
                Color::BLACK.with_alpha(0.).into(),
                Color::BLACK.into(),
            ])
            .in_color_space(InterpolationColorSpace::Srgba)
            .into(),
        ]),
    ))
    .with_children(|p| marker(p, saturation, Some(1. - value)));
    let stops = [0., 60., 120., 180., 240., 300., 360.]
        .map(|h| ColorStop::from(srgb(fields::hsv_to_rgb(h, 1., 1.))))
        .to_vec();
    p.spawn((
        Button,
        control(Action::CloseMenu, "Hue", ControlKind::Picker(false)),
        Node {
            height: px(12),
            border_radius: BorderRadius::all(px(6)),
            flex_shrink: 0.,
            position_type: PositionType::Relative,
            ..default()
        },
        BackgroundGradient::from(
            LinearGradient::to_right(stops).in_color_space(InterpolationColorSpace::Srgba),
        ),
        BorderColor::all(t.border),
    ))
    .with_children(|p| marker(p, hue / 360., None));
}

/// Palette swatches; the matching color is ringed.
pub fn palette(p: Kids, cx: &Ctx, current: [f32; 3]) {
    let t = &cx.t;
    p.spawn(Node {
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(4),
        row_gap: px(4),
        flex_shrink: 0.,
        ..default()
    })
    .with_children(|p| {
        for (i, c) in PALETTE.iter().enumerate() {
            let selected = c.iter().zip(current).all(|(a, b)| (a - b).abs() < 0.002);
            let color = Color::srgb(c[0], c[1], c[2]);
            p.spawn((
                Button,
                control(
                    Action::Color(i),
                    format!("Color {}", i + 1),
                    ControlKind::Button,
                ),
                Node {
                    width: px(20),
                    height: px(20),
                    border: UiRect::all(px(if selected { 2 } else { 1 })),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(color),
                BorderColor::all(if selected { Color::WHITE } else { t.divider }),
                HoverStyle {
                    idle: color,
                    hover: color.lighter(0.08),
                    press: color,
                },
            ));
        }
    });
}

pub fn hint(p: Kids, cx: &Ctx, s: impl Into<String>) {
    p.spawn((
        Node {
            flex_shrink: 0.,
            ..default()
        },
        children![wrapped(cx, s, size::SM, cx.t.text_muted)],
    ));
}

/// A colored status line (validation results, warnings).
pub fn status(p: Kids, cx: &Ctx, ok: bool, s: impl Into<String>) {
    let t = &cx.t;
    let color = if ok { t.success } else { t.warning };
    p.spawn((
        Node {
            padding: UiRect::all(px(8)),
            column_gap: px(8),
            border: UiRect::left(px(3)),
            border_radius: BorderRadius::all(px(4)),
            flex_shrink: 0.,
            ..default()
        },
        BackgroundColor(color.with_alpha(0.1)),
        BorderColor::all(color),
    ))
    .with_children(|p| {
        p.spawn(glyph(cx, if ok { "✓" } else { "⚠" }, 12., color));
        p.spawn((
            Node {
                flex_grow: 1.,
                flex_basis: px(0),
                ..default()
            },
            children![wrapped(cx, s, size::SM, t.text)],
        ));
    });
}

/// Read-only "label: value" text.
pub fn readout(p: Kids, cx: &Ctx, name: &str, value: impl Into<String>) {
    let value = value.into();
    row(p, cx, name, |p| {
        p.spawn(text(cx, value, size::MD, cx.t.text, Weight::Regular));
    });
}

/// A scrollable column with a slim scrollbar; its offset survives rebuilds.
pub fn scroll(p: Kids, cx: &Ctx, key: &str, padding: UiRect, gap: f32, body: impl FnOnce(Kids)) {
    let offset = cx.scroll.0.get(key).copied().unwrap_or(0.);
    p.spawn(Node {
        flex_grow: 1.,
        flex_basis: px(0),
        min_height: px(0),
        position_type: PositionType::Relative,
        ..default()
    })
    .with_children(|p| {
        p.spawn((
            ScrollArea(key.to_string()),
            ScrollPosition(Vec2::new(0., offset)),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                overflow: Overflow::scroll_y(),
                padding,
                row_gap: px(gap),
                ..default()
            },
        ))
        .with_children(body);
        p.spawn((
            ScrollThumb(key.to_string()),
            Node {
                position_type: PositionType::Absolute,
                right: px(2),
                top: px(0),
                width: px(4),
                height: percent(0),
                border_radius: BorderRadius::all(px(2)),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.14)),
            Visibility::Hidden,
        ));
    });
}

/// Centered placeholder with a large glyph, title, explanation and actions.
pub fn empty_state(
    p: Kids,
    cx: &Ctx,
    g: &str,
    title: &str,
    body: &str,
    actions: impl FnOnce(Kids),
) {
    let t = &cx.t;
    p.spawn(Node {
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        row_gap: px(8),
        padding: UiRect::all(px(16)),
        flex_shrink: 0.,
        ..default()
    })
    .with_children(|p| {
        p.spawn(glyph(cx, g, 30., t.text_muted));
        p.spawn(text(cx, title, size::LG, t.text, Weight::Semibold));
        p.spawn((
            Node {
                max_width: px(260),
                margin: UiRect::bottom(px(6)),
                ..default()
            },
            children![(
                wrapped(cx, body, size::SM, t.text_dim),
                TextLayout::justify(Justify::Center),
            )],
        ));
        p.spawn(Node {
            flex_direction: FlexDirection::Column,
            row_gap: px(6),
            align_items: AlignItems::Stretch,
            min_width: px(180),
            ..default()
        })
        .with_children(actions);
    });
}

/// An invisible drag handle on a panel edge; it lights up when hovered.
pub fn splitter(p: Kids, cx: &Ctx, which: Splitter) {
    let mut node = Node {
        position_type: PositionType::Absolute,
        ..default()
    };
    match which {
        Splitter::Left => {
            node.right = px(-3);
            node.top = px(0);
            node.bottom = px(0);
            node.width = px(6);
        }
        Splitter::Right => {
            node.left = px(-3);
            node.top = px(0);
            node.bottom = px(0);
            node.width = px(6);
        }
        Splitter::Explorer => {
            node.top = px(-3);
            node.left = px(0);
            node.right = px(0);
            node.height = px(6);
        }
    }
    p.spawn((
        Button,
        control(Action::CloseMenu, "Resize", ControlKind::Splitter(which)),
        node,
        BackgroundColor(Color::NONE),
        HoverStyle {
            idle: Color::NONE,
            hover: cx.t.accent.with_alpha(0.55),
            press: cx.t.accent,
        },
        ZIndex(30),
    ));
}
