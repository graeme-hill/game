//! Editor UI. The whole tree is rebuilt from [`Editor`] state whenever its
//! revision changes; small per-frame systems handle hover, scrollbars,
//! tooltips and live values (animation time) without rebuilding.
mod explorer;
mod frame;
mod inspector;
pub mod manifest;
mod overlays;
mod rules;
mod start;
pub mod theme;
mod timeline;
mod widgets;

use super::*;
use input::EditorPointer;
use theme::{Fonts, Palette};

/// A clickable UI element and what it does.
#[derive(Component, Clone)]
pub struct Control {
    pub action: Action,
    pub label: String,
    pub kind: ControlKind,
    pub context: Option<Menu>,
    pub tooltip: String,
    /// What a middle-click does (closing a tab).
    pub middle: Option<Action>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
    Button,
    MenuItem,
    /// Drag to scrub, click to type.
    Field(Field),
    /// Click to type.
    Text(TextTarget),
    Timeline,
    /// A keyframe marker: drag to retime, click to jump to it. `bone: None`
    /// moves every track's key at that time.
    Key {
        bone: Option<u32>,
        time: f32,
    },
    /// Drag to resize panels.
    Splitter(Splitter),
    /// The color picker's saturation/value square (`true`) or hue strip.
    Picker(bool),
}

/// Non-interactive UI that still keeps the pointer away from the viewport
/// and from controls underneath it (menus, dialogs, panels).
#[derive(Component)]
#[require(Interaction, bevy::ui::FocusPolicy::Block)]
pub struct UiBlocker;

#[derive(Component)]
pub struct EditorUi;

/// Background colors for idle/hovered/pressed states.
#[derive(Component, Clone, Copy)]
pub struct HoverStyle {
    pub idle: Color,
    pub hover: Color,
    pub press: Color,
}

/// Scrollable pane, keyed so its position survives rebuilds.
#[derive(Component, Clone)]
pub struct ScrollArea(pub String);
#[derive(Component)]
pub struct ScrollThumb(pub String);
#[derive(Resource, Default)]
pub struct ScrollMemory(pub HashMap<String, f32>);

/// Values that change every frame during playback.
#[derive(Component, Clone, Copy, PartialEq)]
pub enum LiveLabel {
    Time,
    Playhead,
}

#[derive(Component)]
pub struct TooltipNode;

/// A row that should scroll into view when the active document or selection
/// changes (the open resource in the Explorer, the selection in the Outline).
#[derive(Component)]
pub struct Reveal;

/// Scroll panes so newly focused rows are visible, without fighting the user's
/// own scrolling afterwards.
pub fn reveal_rows(
    e: Res<Editor>,
    rows: Query<(Entity, &ComputedNode, &UiGlobalTransform), With<Reveal>>,
    parents: Query<&ChildOf>,
    mut areas: Query<(
        &ScrollArea,
        &ComputedNode,
        &UiGlobalTransform,
        &mut ScrollPosition,
    )>,
    mut memory: ResMut<ScrollMemory>,
    mut last: Local<Option<(Option<DocRef>, Sel)>>,
) {
    let focus = (e.active, e.sel());
    if *last == Some(focus) || rows.is_empty() {
        return;
    }
    *last = Some(focus);
    for (row, node, transform) in &rows {
        let rect = Rect::from_center_size(transform.translation, node.size());
        if rect.height() <= 0. {
            continue;
        }
        let mut current = row;
        while let Ok(parent) = parents.get(current) {
            current = parent.parent();
            let Ok((area, area_node, area_transform, mut position)) = areas.get_mut(current) else {
                continue;
            };
            let view = Rect::from_center_size(area_transform.translation, area_node.size());
            let scale = area_node.inverse_scale_factor();
            let delta = if rect.min.y < view.min.y {
                rect.min.y - view.min.y - 8. / scale
            } else if rect.max.y > view.max.y {
                rect.max.y - view.max.y + 8. / scale
            } else {
                0.
            };
            if delta != 0. {
                let max = ((area_node.content_size().y - area_node.size().y) * scale).max(0.);
                let next = (position.0.y + delta * scale).clamp(0., max);
                position.0.y = next;
                memory.0.insert(area.0.clone(), next);
            }
            break;
        }
    }
}

/// Everything a builder needs to read while spawning UI.
pub struct Ctx<'a> {
    pub e: &'a Editor,
    pub fonts: &'a Fonts,
    pub scroll: &'a ScrollMemory,
    pub t: Palette,
    /// Window size in logical (UI-scaled) pixels.
    pub size: Vec2,
    /// Outline rows in display order, collected while building.
    pub outline: std::cell::RefCell<Vec<Sel>>,
}

/// The Outline's selectable rows in display order, for keyboard navigation.
#[derive(Resource, Default)]
pub struct OutlineOrder(pub Vec<Sel>);

impl Ctx<'_> {
    /// Panel widths yield to the viewport in small windows.
    pub fn left_width(&self) -> f32 {
        if !self.e.layout.show_left {
            return 0.;
        }
        self.e.layout.left.min(self.size.x * 0.26).max(170.)
    }
    pub fn right_width(&self) -> f32 {
        if !self.e.layout.show_right {
            return 0.;
        }
        self.e.layout.right.min(self.size.x * 0.3).max(210.)
    }
    pub fn center_width(&self) -> f32 {
        self.size.x - self.left_width() - self.right_width()
    }
}

/// UI scale follows the client area so small windows stay usable and large
/// ones gain room as well as slightly larger text.
pub fn responsive_scale(windows: Query<&Window>, mut ui_scale: ResMut<UiScale>) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let fit = (window.resolution.physical_width() as f32 / 1280.)
        .min(window.resolution.physical_height() as f32 / 720.)
        .max(0.01);
    let scale = fit.sqrt().clamp(0.62, 1.5);
    if (ui_scale.0 - scale).abs() > 0.001 {
        ui_scale.0 = scale;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn rebuild_ui(
    mut commands: Commands,
    e: Res<Editor>,
    fonts: Res<Fonts>,
    scroll: Res<ScrollMemory>,
    old: Query<Entity, With<EditorUi>>,
    windows: Query<&Window>,
    ui_scale: Res<UiScale>,
    mut order: ResMut<OutlineOrder>,
    mut last: Local<(u64, Vec2)>,
) {
    let size = windows.iter().next().map_or(Vec2::new(1280., 720.), |w| {
        Vec2::new(
            w.resolution.physical_width() as f32,
            w.resolution.physical_height() as f32,
        ) / ui_scale.0.max(0.01)
    });
    // Rebuild on edits, and when the window size changes enough to reflow.
    if last.0 == e.revision && (last.1 - size).abs().max_element() < 24. {
        return;
    }
    *last = (e.revision, size);
    for entity in &old {
        commands.entity(entity).despawn();
    }
    let cx = Ctx {
        e: &e,
        fonts: &fonts,
        scroll: &scroll,
        t: Palette::get(),
        size,
        outline: Default::default(),
    };
    commands
        .spawn((
            EditorUi,
            // No background: the 3D viewport shows through the center.
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Column,
                ..default()
            },
        ))
        .with_children(|root| frame::build(root, &cx));
    order.0 = cx.outline.take();
}

type HoverChanged = Or<(Changed<Interaction>, Added<HoverStyle>)>;

pub fn hover_styles(
    mut nodes: Query<(&Interaction, &HoverStyle, &mut BackgroundColor), HoverChanged>,
) {
    for (interaction, style, mut background) in &mut nodes {
        let color = match interaction {
            Interaction::Pressed => style.press,
            Interaction::Hovered => style.hover,
            Interaction::None => style.idle,
        };
        if background.0 != color {
            background.0 = color;
        }
    }
}

pub fn live_labels(
    e: Res<Editor>,
    mut texts: Query<(&LiveLabel, Option<&mut Text>, Option<&mut Node>)>,
) {
    let Some(clip) = e.animation_clip() else {
        return;
    };
    for (label, text, node) in &mut texts {
        match label {
            LiveLabel::Time => {
                if let Some(mut text) = text {
                    let value = format!("{:.2}s", e.anim.time);
                    if text.0 != value {
                        text.0 = value;
                    }
                }
            }
            LiveLabel::Playhead => {
                if let Some(mut node) = node {
                    let left = percent(e.anim.time / clip.duration.max(0.001) * 100.);
                    if node.left != left {
                        node.left = left;
                    }
                }
            }
        }
    }
}

/// Position each pane's scrollbar thumb from its content and scroll offset.
pub fn scrollbars(
    areas: Query<(&ScrollArea, &ComputedNode, &ScrollPosition)>,
    mut thumbs: Query<(&ScrollThumb, &mut Node, &mut Visibility)>,
) {
    for (thumb, mut node, mut visibility) in &mut thumbs {
        let Some((_, computed, position)) = areas.iter().find(|(a, ..)| a.0 == thumb.0) else {
            continue;
        };
        let content = computed.content_size().y * computed.inverse_scale_factor();
        let size = computed.size().y * computed.inverse_scale_factor();
        let shown = content > size + 1.;
        let wanted = if shown {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != wanted {
            *visibility = wanted;
        }
        if shown {
            let height = percent((size / content * 100.).max(8.));
            let top = percent(position.0.y / content * 100.);
            if node.height != height {
                node.height = height;
            }
            if node.top != top {
                node.top = top;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn tooltips(
    mut commands: Commands,
    pointer: Res<EditorPointer>,
    editor: Res<Editor>,
    time: Res<Time>,
    controls: Query<(&Control, &ComputedNode, &UiGlobalTransform)>,
    existing: Query<(Entity, &TooltipNode)>,
    fonts: Res<Fonts>,
    ui_scale: Res<UiScale>,
    windows: Query<&Window>,
    mut shown: Local<Option<Entity>>,
) {
    let target = pointer
        .hovered
        .filter(|(_, since)| time.elapsed_secs() - since > 0.55)
        .filter(|_| pointer.ui_drag.is_none() && editor.menu.is_none() && editor.modal.is_none())
        .map(|(entity, _)| entity);
    if *shown == target && (target.is_none() || !existing.is_empty()) {
        return;
    }
    for (entity, _) in &existing {
        commands.entity(entity).despawn();
    }
    *shown = target;
    let Some(entity) = target else { return };
    let Ok((control, node, transform)) = controls.get(entity) else {
        return;
    };
    let t = Palette::get();
    let scale = ui_scale.0.max(0.01);
    let rect = Rect::from_center_size(transform.translation, node.size());
    let window_width = windows
        .iter()
        .next()
        .map_or(1280., |w| w.resolution.physical_width() as f32);
    let width_guess = (control.tooltip.chars().count() as f32 * 6.4 + 20.) * scale;
    let left = (rect.min.x).min(window_width - width_guess - 8.).max(4.) / scale;
    commands.spawn((
        TooltipNode,
        Node {
            position_type: PositionType::Absolute,
            left: px(left),
            top: px(rect.max.y / scale + 6.),
            padding: UiRect::axes(px(8), px(5)),
            border: UiRect::all(px(1)),
            border_radius: BorderRadius::all(px(4)),
            ..default()
        },
        BackgroundColor(t.raised),
        BorderColor::all(t.border),
        BoxShadow::new(t.shadow, px(0), px(3), px(0), px(8)),
        GlobalZIndex(100),
        children![(
            Text::new(control.tooltip.clone()),
            TextFont {
                font: FontSource::Handle(fonts.regular.clone()),
                font_size: FontSize::Px(theme::size::SM),
                ..default()
            },
            TextColor(t.text),
        )],
    ));
}

#[derive(Component)]
pub struct HoverLabel;

/// Name what the pointer is over in the viewport, next to the pointer.
#[allow(clippy::too_many_arguments)]
pub fn hover_label(
    mut commands: Commands,
    e: Res<Editor>,
    hover: Res<viewport::ViewportHover>,
    pointer: Res<EditorPointer>,
    windows: Query<&Window>,
    ui_scale: Res<UiScale>,
    fonts: Res<Fonts>,
    mut labels: Query<(Entity, &mut Node, &Children), With<HoverLabel>>,
    mut texts: Query<&mut Text>,
) {
    let cursor = windows.iter().next().and_then(Window::cursor_position);
    let label = hover
        .0
        .filter(|_| pointer.viewport_drag.is_none() && e.menu.is_none())
        .and_then(|sel| e.sel_label(sel))
        .zip(cursor);
    let existing = labels.iter_mut().next();
    match (label, existing) {
        (Some((name, p)), Some((_, mut node, children))) => {
            let scale = ui_scale.0.max(0.01);
            let (left, top) = (px(p.x / scale + 14.), px(p.y / scale + 16.));
            if node.left != left || node.top != top || node.display != Display::Flex {
                node.left = left;
                node.top = top;
                node.display = Display::Flex;
            }
            if let Some(mut text) = children.first().and_then(|c| texts.get_mut(*c).ok())
                && text.0 != name
            {
                text.0 = name;
            }
        }
        (Some((name, p)), None) => {
            let t = Palette::get();
            let scale = ui_scale.0.max(0.01);
            commands.spawn((
                HoverLabel,
                Node {
                    position_type: PositionType::Absolute,
                    left: px(p.x / scale + 14.),
                    top: px(p.y / scale + 16.),
                    padding: UiRect::axes(px(7), px(3)),
                    border_radius: BorderRadius::all(px(4)),
                    ..default()
                },
                BackgroundColor(t.bg.with_alpha(0.85)),
                GlobalZIndex(90),
                children![(
                    Text::new(name),
                    TextFont {
                        font: FontSource::Handle(fonts.medium.clone()),
                        font_size: FontSize::Px(theme::size::SM),
                        ..default()
                    },
                    TextColor(t.text),
                )],
            ));
        }
        (None, Some((_, mut node, _))) => {
            if node.display != Display::None {
                node.display = Display::None;
            }
        }
        (None, None) => {}
    }
}
