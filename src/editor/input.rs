//! Pointer and keyboard input for UI controls. Viewport tools live in
//! `viewport`; both consult [`EditorPointer`] so a press is handled once.
use super::ui::{Control, ControlKind, ScrollArea, ScrollMemory, UiBlocker};
use super::*;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardFocusLost, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    window::PrimaryWindow,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiDrag {
    /// Dragging horizontally scrubs a numeric field; a click starts typing.
    Field {
        field: Field,
        start_x: f32,
        start_value: f32,
        moved: bool,
    },
    /// Scrubbing the animation timeline.
    Timeline { left: f32, width: f32 },
    /// Dragging a keyframe along the timeline.
    Key {
        bone: Option<u32>,
        from: f32,
        current: f32,
        start_x: f32,
        moved: bool,
        left: f32,
        width: f32,
    },
    /// Choosing a color in the picker square (`square`) or hue strip.
    Picker { square: bool, rect: Rect },
    /// Resizing a panel; `start` is the cursor coordinate along the drag axis.
    Split {
        which: Splitter,
        start: f32,
        value: f32,
        extent: f32,
    },
}

#[derive(Resource, Default)]
pub struct EditorPointer {
    /// The cursor is over UI that blocks viewport input.
    pub over_ui: bool,
    pub ui_drag: Option<UiDrag>,
    pub camera_drag: Option<MouseButton>,
    pub viewport_drag: Option<viewport::ViewportDrag>,
    pub last_cursor: Option<Vec2>,
    /// The hovered control with a tooltip and when hovering began.
    pub hovered: Option<(Entity, f32)>,
    /// A hovered "place this tile here" menu entry, previewed in 3D.
    pub preview: Option<(u32, usize)>,
    /// The last clicked control's action and when, for double-clicks.
    pub last_click: Option<(Action, f32)>,
}

#[allow(clippy::too_many_arguments)]
pub fn controls(
    controls: Query<(
        Entity,
        &Interaction,
        &Control,
        &ComputedNode,
        &UiGlobalTransform,
    )>,
    blockers: Query<&Interaction, With<UiBlocker>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    time: Res<Time>,
    ui_scale: Res<UiScale>,
    mut pointer: ResMut<EditorPointer>,
    mut e: ResMut<Editor>,
    options: Res<LaunchOptions>,
) {
    let cursor = windows.single().ok().and_then(Window::cursor_position);
    let scale = ui_scale.0.max(0.01);
    pointer.over_ui = controls.iter().any(|(_, i, ..)| *i != Interaction::None)
        || blockers.iter().any(|i| *i != Interaction::None)
        || e.modal.is_some();
    let preview = controls
        .iter()
        .find(|(_, i, c, ..)| **i == Interaction::Hovered && c.kind == ControlKind::MenuItem)
        .and_then(|(_, _, c, ..)| match c.action {
            Action::PlaceOption { tile, turn } => Some((tile, turn)),
            _ => None,
        });
    if pointer.preview != preview {
        pointer.preview = preview;
    }
    // Tooltip hover tracking.
    let hovered = controls
        .iter()
        .find(|(_, i, c, ..)| **i == Interaction::Hovered && !c.tooltip.is_empty())
        .map(|(entity, ..)| entity);
    match (hovered, pointer.hovered) {
        (Some(entity), Some((old, _))) if entity == old => {}
        (Some(entity), _) => pointer.hovered = Some((entity, time.elapsed_secs())),
        (None, _) => pointer.hovered = None,
    }

    // Continue a drag that started on a control.
    if let Some(drag) = pointer.ui_drag {
        let x = cursor.map_or(0., |c| c.x);
        if !mouse.pressed(MouseButton::Left) {
            pointer.ui_drag = None;
            match drag {
                UiDrag::Field {
                    field,
                    moved: false,
                    ..
                } => {
                    // A click without dragging types a value instead.
                    e.end_gesture();
                    e.apply(Action::Edit(TextTarget::Value(field)), &options)
                }
                UiDrag::Field { .. } => e.end_gesture(),
                UiDrag::Timeline { .. } | UiDrag::Split { .. } => e.changed(),
                UiDrag::Picker { .. } => e.end_gesture(),
                UiDrag::Key {
                    from, moved: false, ..
                } => e.apply(Action::SetValue(Field::Time, from), &options),
                UiDrag::Key { .. } => e.end_gesture(),
            }
            return;
        }
        match drag {
            UiDrag::Field {
                field,
                start_x,
                start_value,
                moved,
            } => {
                let dx = (x - start_x) / scale;
                if moved || dx.abs() > 3. {
                    pointer.ui_drag = Some(UiDrag::Field {
                        field,
                        start_x,
                        start_value,
                        moved: true,
                    });
                    let mut value = start_value + dx * field.spec().drag;
                    if field == Field::Blend {
                        value = start_value + dx * 0.005;
                    }
                    let current = fields::get(&e, field);
                    if current.is_some_and(|c| (c - value).abs() > f32::EPSILON) {
                        if field.is_view_state() {
                            let _ = fields::set(&mut e, field, value);
                            if field == Field::Rgb(0)
                                || field == Field::Rgb(1)
                                || field == Field::Rgb(2)
                            {
                                // Bone colors are document edits.
                                e.scene_revision += 1;
                            }
                            e.changed();
                        } else {
                            e.gesture_edit(|e| fields::set(e, field, value));
                        }
                    }
                }
            }
            UiDrag::Split {
                which,
                start,
                value,
                extent,
            } => {
                let y = cursor.map_or(0., |c| c.y);
                let layout = &mut e.layout;
                match which {
                    Splitter::Left => layout.left = (value + (x - start) / scale).clamp(180., 520.),
                    Splitter::Right => {
                        layout.right = (value - (x - start) / scale).clamp(240., 560.)
                    }
                    Splitter::Explorer => {
                        layout.split = (value + (y - start) / extent.max(1.)).clamp(0.15, 0.85)
                    }
                }
                e.changed();
            }
            UiDrag::Key {
                bone,
                from,
                current,
                start_x,
                moved,
                left,
                width,
            } => {
                if !moved && (x - start_x).abs() < 3. {
                    return;
                }
                let Some(duration) = e.animation_clip().map(|c| c.duration) else {
                    return;
                };
                // Keys land on hundredths of a second.
                let t =
                    (((x - left) / width.max(1.)).clamp(0., 1.) * duration * 100.).round() / 100.;
                let mut current = current;
                if (t - current).abs() > 0.0001 {
                    e.gesture_edit(|e| animation::retime(e, bone, current, t));
                    if e.notice.kind != NoticeKind::Error {
                        current = t;
                    }
                }
                pointer.ui_drag = Some(UiDrag::Key {
                    bone,
                    from,
                    current,
                    start_x,
                    moved: true,
                    left,
                    width,
                });
            }
            UiDrag::Picker { square, rect } => {
                if let Some(c) = cursor {
                    pick(&mut e, square, rect, c);
                }
            }
            UiDrag::Timeline { left, width } => {
                if let Some(duration) = e.animation_clip().map(|c| c.duration) {
                    let t = ((x - left) / width.max(1.)).clamp(0., 1.) * duration;
                    if (t - e.anim.time).abs() > 0.0001 {
                        e.anim.time = t;
                        e.anim.playing = false;
                        e.changed();
                    }
                }
            }
        }
        return;
    }

    if mouse.just_pressed(MouseButton::Middle)
        && let Some((_, _, control, ..)) = controls
            .iter()
            .find(|(_, i, c, ..)| **i != Interaction::None && c.middle.is_some())
    {
        e.apply(control.middle.clone().unwrap(), &options);
        return;
    }
    if mouse.just_pressed(MouseButton::Right) {
        if let Some((_, _, control, ..)) = controls
            .iter()
            .find(|(_, i, c, ..)| **i != Interaction::None && c.context.is_some())
            && let Some(p) = cursor
        {
            let menu = control.context.clone().unwrap();
            e.commit_edit();
            // Right-clicking an Outline item selects it first.
            if let Action::Select(sel) = control.action
                && e.sel() != sel
            {
                e.apply(Action::Select(sel), &options);
            }
            e.menu = Some(OpenMenu {
                menu,
                pos: p / scale + Vec2::new(2., 2.),
                width: 200.,
            });
            e.changed();
        } else if e.menu.is_some() {
            e.menu = None;
            e.changed();
        }
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let pressed = controls
        .iter()
        .find(|(_, i, ..)| **i == Interaction::Pressed);
    let Some((_, _, control, node, transform)) = pressed else {
        // A click on empty UI or the viewport closes menus and ends typing.
        if e.menu.is_some() {
            e.menu = None;
            e.changed();
            pointer.over_ui = true;
        }
        if e.editing.is_some() {
            e.commit_edit();
        }
        return;
    };
    let rect = Rect::from_center_size(transform.translation, node.size());
    if e.menu.is_some() && control.kind != ControlKind::MenuItem {
        e.menu = None;
        e.changed();
        // A press on the control that opened the menu just closes it.
        if matches!(control.action, Action::OpenMenu(_)) {
            return;
        }
    }
    match &control.kind {
        ControlKind::Field(field) => {
            if e.editing.as_ref().map(|t| &t.target) == Some(&TextTarget::Value(*field)) {
                return;
            }
            e.commit_edit();
            if let Some(value) = fields::get(&e, *field) {
                if !field.is_view_state() {
                    e.begin_gesture(format!("Edit {}", field.name()));
                }
                pointer.ui_drag = Some(UiDrag::Field {
                    field: *field,
                    start_x: cursor.map_or(rect.center().x, |c| c.x),
                    start_value: value,
                    moved: false,
                });
            }
        }
        ControlKind::Splitter(which) => {
            let c = cursor.unwrap_or(rect.center());
            let layout = e.layout;
            pointer.ui_drag = Some(match which {
                Splitter::Left => UiDrag::Split {
                    which: *which,
                    start: c.x,
                    value: layout.left,
                    extent: 0.,
                },
                Splitter::Right => UiDrag::Split {
                    which: *which,
                    start: c.x,
                    value: layout.right,
                    extent: 0.,
                },
                // The split is a fraction of the left panel's height.
                Splitter::Explorer => UiDrag::Split {
                    which: *which,
                    start: c.y,
                    value: layout.split,
                    extent: windows
                        .single()
                        .map_or(600., |w| w.resolution.physical_height() as f32)
                        - (ui::theme::metric::TOP_BAR + ui::theme::metric::STATUS_BAR) * scale,
                },
            });
        }
        ControlKind::Key { bone, time } => {
            e.commit_edit();
            let timeline = controls
                .iter()
                .find(|(_, _, c, ..)| c.kind == ControlKind::Timeline)
                .map(|(_, _, _, node, t)| Rect::from_center_size(t.translation, node.size()));
            if let Some(timeline) = timeline {
                e.begin_gesture("Move key");
                pointer.ui_drag = Some(UiDrag::Key {
                    bone: *bone,
                    from: *time,
                    current: *time,
                    start_x: cursor.map_or(rect.center().x, |c| c.x),
                    moved: false,
                    left: timeline.min.x,
                    width: timeline.width(),
                });
            }
        }
        ControlKind::Picker(square) => {
            e.commit_edit();
            e.begin_gesture("Color");
            pointer.ui_drag = Some(UiDrag::Picker {
                square: *square,
                rect,
            });
            if let Some(c) = cursor {
                pick(&mut e, *square, rect, c);
            }
        }
        ControlKind::Timeline => {
            e.commit_edit();
            pointer.ui_drag = Some(UiDrag::Timeline {
                left: rect.min.x,
                width: rect.width(),
            });
            if let (Some(duration), Some(c)) = (e.animation_clip().map(|c| c.duration), cursor) {
                e.anim.time = ((c.x - rect.min.x) / rect.width().max(1.)).clamp(0., 1.) * duration;
                e.anim.playing = false;
                e.changed();
            }
        }
        ControlKind::Text(target) => {
            if e.editing.as_ref().map(|t| &t.target) != Some(target) {
                e.apply(Action::Edit(target.clone()), &options);
            }
        }
        ControlKind::Button | ControlKind::MenuItem => {
            if let Action::OpenMenu(menu) = &control.action {
                e.commit_edit();
                e.menu = Some(OpenMenu {
                    menu: menu.clone(),
                    pos: Vec2::new(rect.min.x, rect.max.y) / scale + Vec2::new(0., 3.),
                    width: (rect.width() / scale).max(160.),
                });
                e.changed();
                return;
            }
            if control.kind == ControlKind::MenuItem {
                e.menu = None;
            }
            let now = time.elapsed_secs();
            let double = pointer
                .last_click
                .as_ref()
                .is_some_and(|(action, at)| *action == control.action && now - at < 0.45);
            pointer.last_click = Some((control.action.clone(), now));
            if double && let Some(action) = double_click(&e, &control.action) {
                pointer.last_click = None;
                e.apply(action, &options);
                return;
            }
            e.apply(control.action.clone(), &options);
        }
    }
}

/// Apply a color picker position (window coordinates).
fn pick(e: &mut Editor, square: bool, rect: Rect, cursor: Vec2) {
    let at = ((cursor - rect.min) / rect.size().max(Vec2::ONE)).clamp(Vec2::ZERO, Vec2::ONE);
    let (hue, saturation, value) = fields::rgb_to_hsv(fields::current_color(e));
    let hue = if saturation < 0.001 || value < 0.001 {
        e.hue
    } else {
        hue
    };
    let rgb = if square {
        fields::hsv_to_rgb(hue, at.x, 1. - at.y)
    } else {
        e.hue = at.x * 360.;
        fields::hsv_to_rgb(e.hue, saturation.max(0.05), value.max(0.05))
    };
    if square {
        e.hue = hue;
    }
    if e.active_kind() == Some(DocKind::Body) {
        e.gesture_edit(|e| fields::set_color(e, rgb));
    } else {
        let _ = fields::set_color(e, rgb);
        e.changed();
    }
}

/// What double-clicking an Explorer or Outline row does: rename it (or open
/// a world piece's tile).
fn double_click(e: &Editor, action: &Action) -> Option<Action> {
    Some(match action {
        Action::Open(doc) if doc.kind != DocKind::Rules => Action::Rename(*doc),
        Action::Select(sel) => match (e.active_kind()?, sel) {
            (DocKind::Body, Sel::Bone(_)) => Action::Edit(TextTarget::BoneName),
            (_, Sel::Mount(_)) => Action::Edit(TextTarget::MountName),
            (_, Sel::Anchor(_)) => Action::Edit(TextTarget::AnchorName),
            (_, Sel::Clip(_)) => Action::Edit(TextTarget::ClipName),
            (_, Sel::Socket(_)) => Action::Edit(TextTarget::SocketName),
            (_, Sel::Piece(id)) => {
                let tile = e
                    .active_world()?
                    .instances
                    .iter()
                    .find(|i| i.id == *id)?
                    .tile;
                Action::Open(DocRef {
                    kind: DocKind::Tile,
                    id: tile,
                })
            }
            _ => return None,
        },
        _ => return None,
    })
}

/// Keyboard: text entry when a field is being edited, otherwise shortcuts.
pub fn keyboard(
    mut events: MessageReader<KeyboardInput>,
    mut focus_lost: MessageReader<KeyboardFocusLost>,
    mut modifiers: Local<[bool; 4]>,
    mut e: ResMut<Editor>,
    options: Res<LaunchOptions>,
    order: Option<Res<ui::OutlineOrder>>,
) {
    if focus_lost.read().next().is_some() {
        *modifiers = [false; 4];
    }
    // Track modifiers from the event stream so chords pressed and released
    // within one frame keep their order.
    for event in events.read() {
        let pressed = event.state == ButtonState::Pressed;
        match event.key_code {
            KeyCode::ControlLeft => modifiers[0] = pressed,
            KeyCode::ControlRight => modifiers[1] = pressed,
            KeyCode::ShiftLeft => modifiers[2] = pressed,
            KeyCode::ShiftRight => modifiers[3] = pressed,
            _ => {}
        }
        if !pressed {
            continue;
        }
        let ctrl = modifiers[0] || modifiers[1];
        let shift = modifiers[2] || modifiers[3];
        if e.editing.is_some() {
            text_key(&mut e, event, ctrl, &options);
        } else {
            let order = order.as_ref().map_or(&[][..], |o| o.0.as_slice());
            shortcut(&mut e, event.key_code, ctrl, shift, &options, order);
        }
    }
}

fn text_key(e: &mut Editor, event: &KeyboardInput, ctrl: bool, options: &LaunchOptions) {
    match event.key_code {
        KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Tab => {
            e.apply(Action::CommitEdit, options);
            return;
        }
        KeyCode::Escape => {
            e.apply(Action::CancelEdit, options);
            return;
        }
        KeyCode::KeyS if ctrl => {
            e.apply(Action::CommitEdit, options);
            e.apply(Action::Save, options);
            return;
        }
        _ => {}
    }
    if let Some(TextTarget::Value(field)) = e.editing.as_ref().map(|t| t.target.clone())
        && matches!(event.key_code, KeyCode::ArrowUp | KeyCode::ArrowDown)
    {
        // Arrows step a number while typing in it.
        let step = field.spec().step;
        let delta = if event.key_code == KeyCode::ArrowUp {
            step
        } else {
            -step
        };
        e.apply(Action::Nudge(field, delta), options);
        e.begin_edit(TextTarget::Value(field));
        return;
    }
    let Some(edit) = e.editing.as_mut() else {
        return;
    };
    let len = edit.buffer.chars().count();
    edit.caret = edit.caret.min(len);
    let byte = |s: &str, chars: usize| s.char_indices().nth(chars).map_or(s.len(), |(i, _)| i);
    match event.key_code {
        KeyCode::Backspace | KeyCode::Delete if edit.fresh => {
            edit.buffer.clear();
            edit.caret = 0;
            edit.fresh = false;
        }
        KeyCode::Backspace => {
            if edit.caret > 0 {
                let at = byte(&edit.buffer, edit.caret - 1);
                edit.buffer.remove(at);
                edit.caret -= 1;
            }
        }
        KeyCode::Delete => {
            if edit.caret < len {
                let at = byte(&edit.buffer, edit.caret);
                edit.buffer.remove(at);
            }
        }
        // With everything selected, arrows collapse the selection to an end.
        KeyCode::ArrowLeft => {
            edit.caret = if edit.fresh {
                0
            } else {
                edit.caret.saturating_sub(1)
            };
            edit.fresh = false;
        }
        KeyCode::ArrowRight => {
            edit.caret = if edit.fresh {
                len
            } else {
                (edit.caret + 1).min(len)
            };
            edit.fresh = false;
        }
        KeyCode::Home => {
            edit.caret = 0;
            edit.fresh = false;
        }
        KeyCode::End => {
            edit.caret = len;
            edit.fresh = false;
        }
        KeyCode::KeyA if ctrl => edit.fresh = true,
        _ if !ctrl => {
            let text = event
                .text
                .as_ref()
                .map(|t| t.to_string())
                .or(match &event.logical_key {
                    Key::Space => Some(" ".into()),
                    _ => None,
                });
            if let Some(text) = text {
                if edit.fresh {
                    edit.buffer.clear();
                    edit.caret = 0;
                    edit.fresh = false;
                }
                let limit = edit.target.limit();
                for c in text.chars().filter(|c| !c.is_control()) {
                    if edit.buffer.chars().count() < limit {
                        let at = byte(&edit.buffer, edit.caret);
                        edit.buffer.insert(at, c);
                        edit.caret += 1;
                    }
                }
            }
        }
        _ => return,
    }
    if edit.target == TextTarget::Filter {
        e.filter = edit.buffer.clone();
    }
    e.changed();
}

fn shortcut(
    e: &mut Editor,
    key: KeyCode,
    ctrl: bool,
    shift: bool,
    options: &LaunchOptions,
    outline: &[Sel],
) {
    let kind = e.active_kind();
    let voxels = matches!(kind, Some(DocKind::Prop | DocKind::Tile));
    let action = match key {
        KeyCode::KeyS if ctrl => Action::Save,
        KeyCode::KeyZ if ctrl && shift => Action::Redo,
        KeyCode::KeyZ if ctrl => Action::Undo,
        KeyCode::KeyY if ctrl => Action::Redo,
        KeyCode::KeyO if ctrl => Action::ShowWorkspaceBrowser,
        KeyCode::KeyQ if ctrl => Action::Quit,
        KeyCode::KeyN if ctrl => {
            e.menu = Some(OpenMenu {
                menu: Menu::New,
                pos: Vec2::new(8., 40.),
                width: 200.,
            });
            e.changed();
            return;
        }
        KeyCode::KeyW if ctrl => match e.active {
            Some(doc) => Action::CloseTab(doc),
            None => return,
        },
        // Duplicate the selection when there is one, else the document.
        KeyCode::KeyD if ctrl => match (e.sel(), e.active) {
            (Sel::None | Sel::Piece(_) | Sel::PieceSocket(..) | Sel::Attachment(_), Some(doc))
                if doc.kind != DocKind::Rules =>
            {
                Action::Duplicate(doc)
            }
            (Sel::None | Sel::Piece(_) | Sel::PieceSocket(..) | Sel::Attachment(_), _) => return,
            _ => Action::DuplicateSelection,
        },
        KeyCode::Tab if ctrl => {
            let Some(index) = e.active.and_then(|a| e.tabs.iter().position(|t| *t == a)) else {
                return;
            };
            let n = e.tabs.len();
            let next = if shift { index + n - 1 } else { index + 1 } % n;
            Action::Open(e.tabs[next])
        }
        _ if ctrl => return,
        KeyCode::Escape => {
            if e.menu.is_some() {
                Action::CloseMenu
            } else if e.modal.is_some() {
                Action::CloseModal
            } else if e.sel() != Sel::None {
                Action::Select(Sel::None)
            } else {
                return;
            }
        }
        KeyCode::F2 => match (e.sel(), e.active) {
            (Sel::Bone(_), Some(d)) if d.kind == DocKind::Body => {
                Action::Edit(TextTarget::BoneName)
            }
            (Sel::Mount(_), _) => Action::Edit(TextTarget::MountName),
            (Sel::Anchor(_), _) => Action::Edit(TextTarget::AnchorName),
            (Sel::Clip(_), _) => Action::Edit(TextTarget::ClipName),
            (Sel::Socket(_), _) => Action::Edit(TextTarget::SocketName),
            (_, Some(doc)) if doc.kind != DocKind::Rules => Action::Rename(doc),
            _ => return,
        },
        KeyCode::Delete | KeyCode::Backspace if e.sel() != Sel::None => Action::DeleteSelection,
        KeyCode::F5 => Action::Play,
        KeyCode::F1 => Action::ShowShortcuts,
        KeyCode::Slash if shift => Action::ShowShortcuts,
        KeyCode::ArrowUp | KeyCode::ArrowDown if !outline.is_empty() => {
            // Step through the Outline like a tree view.
            let current = outline.iter().position(|s| *s == e.sel());
            let next = match (current, key == KeyCode::ArrowDown) {
                (Some(i), true) => (i + 1).min(outline.len() - 1),
                (Some(i), false) => i.saturating_sub(1),
                (None, _) => 0,
            };
            Action::Select(outline[next])
        }
        KeyCode::KeyF => Action::Frame,
        KeyCode::KeyG => Action::ToggleGuides,
        KeyCode::Backslash => Action::FocusViewport,
        KeyCode::Digit1 | KeyCode::Numpad1 => Action::View(actions::ViewPreset::Front),
        KeyCode::Digit3 | KeyCode::Numpad3 => Action::View(actions::ViewPreset::Side),
        KeyCode::Digit7 | KeyCode::Numpad7 => Action::View(actions::ViewPreset::Top),
        KeyCode::Digit0 | KeyCode::Numpad0 => Action::View(actions::ViewPreset::Perspective),
        KeyCode::KeyB if voxels => Action::SetTool(Tool::Paint),
        KeyCode::KeyE if voxels => Action::SetTool(Tool::Erase),
        KeyCode::KeyV if kind == Some(DocKind::Prop) => Action::SetTool(Tool::Select),
        KeyCode::KeyM if voxels => Action::ToggleMirror,
        KeyCode::BracketLeft | KeyCode::BracketRight if voxels => {
            let i = BRUSHES.iter().position(|b| *b == e.brush).unwrap_or(0);
            let i = if key == KeyCode::BracketLeft {
                i.saturating_sub(1)
            } else {
                (i + 1).min(BRUSHES.len() - 1)
            };
            Action::Brush(BRUSHES[i])
        }
        KeyCode::Space if e.anim.clip.is_some() => Action::Anim(AnimOp::Play),
        KeyCode::KeyK if e.anim.clip.is_some() => Action::Anim(AnimOp::Key),
        KeyCode::Comma if e.anim.clip.is_some() => Action::Anim(AnimOp::PrevKey),
        KeyCode::ArrowLeft if e.anim.clip.is_some() => Action::Anim(AnimOp::Step(-1. / 30.)),
        KeyCode::ArrowRight if e.anim.clip.is_some() => Action::Anim(AnimOp::Step(1. / 30.)),
        KeyCode::Period if e.anim.clip.is_some() => Action::Anim(AnimOp::NextKey),
        _ => return,
    };
    e.apply(action, options);
}

/// Mouse-wheel scrolling of explorer/inspector panes. Positions persist
/// across UI rebuilds through [`ScrollMemory`].
pub fn scroll_panes(
    mut wheel: MessageReader<MouseWheel>,
    windows: Query<&Window, With<PrimaryWindow>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut panes: Query<(
        &ScrollArea,
        &ComputedNode,
        &UiGlobalTransform,
        &mut ScrollPosition,
    )>,
    mut memory: ResMut<ScrollMemory>,
) {
    let delta: f32 = wheel
        .read()
        .map(|w| match w.unit {
            MouseScrollUnit::Line => w.y * 40.,
            MouseScrollUnit::Pixel => w.y,
        })
        .sum();
    if delta == 0. {
        return;
    }
    let Some(cursor) = windows.single().ok().and_then(Window::cursor_position) else {
        return;
    };
    // Shift scrolls sideways. The innermost pane that can move along the
    // requested axis wins; a sideways-only pane also takes the plain wheel.
    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let mut under: Vec<_> = panes
        .iter_mut()
        .filter_map(|(area, node, transform, position)| {
            let rect = Rect::from_center_size(transform.translation, node.size());
            rect.contains(cursor)
                .then_some((rect.width() * rect.height(), area, node, position))
        })
        .collect();
    under.sort_by(|a, b| a.0.total_cmp(&b.0));
    let room = |node: &ComputedNode| {
        ((node.content_size() - node.size()) * node.inverse_scale_factor()).max(Vec2::ZERO)
    };
    let pick = |horizontal: bool| {
        under.iter().position(|(_, _, node, _)| {
            let r = room(node);
            if horizontal { r.x > 0.5 } else { r.y > 0.5 }
        })
    };
    let (index, horizontal) = match (shift, pick(shift)) {
        (_, Some(i)) => (i, shift),
        (false, None) => match pick(true) {
            Some(i) => (i, true),
            None => return,
        },
        (true, None) => return,
    };
    let (_, area, node, position) = &mut under[index];
    let max = room(node);
    if horizontal {
        let next = (position.0.x - delta).clamp(0., max.x);
        if (next - position.0.x).abs() > 0.01 {
            position.0.x = next;
            memory.0.insert(format!("{}:x", area.0), next);
        }
    } else {
        let next = (position.0.y - delta).clamp(0., max.y);
        if (next - position.0.y).abs() > 0.01 {
            position.0.y = next;
            memory.0.insert(area.0.clone(), next);
        }
    }
}

/// Pointer shape follows what is under it: resize handles, scrub fields and
/// text inputs each get the conventional cursor.
pub fn cursor_icons(
    mut commands: Commands,
    windows: Query<(Entity, Option<&bevy::window::CursorIcon>), With<PrimaryWindow>>,
    controls: Query<(&Interaction, &Control)>,
    pointer: Res<EditorPointer>,
    gizmo: Res<super::gizmo::GizmoHover>,
    target: Res<viewport::ViewportHover>,
) {
    use bevy::window::{CursorIcon, SystemCursorIcon as Icon};
    let kind_icon = |kind: &ControlKind| match kind {
        ControlKind::Splitter(Splitter::Explorer) => Some(Icon::RowResize),
        ControlKind::Splitter(_) => Some(Icon::ColResize),
        ControlKind::Field(_) | ControlKind::Key { .. } => Some(Icon::EwResize),
        ControlKind::Picker(_) => Some(Icon::Crosshair),
        ControlKind::Text(_) => Some(Icon::Text),
        _ => None,
    };
    let icon = match pointer.ui_drag {
        Some(UiDrag::Split {
            which: Splitter::Explorer,
            ..
        }) => Icon::RowResize,
        Some(UiDrag::Split { .. }) => Icon::ColResize,
        Some(UiDrag::Field { .. }) => Icon::EwResize,
        Some(UiDrag::Timeline { .. } | UiDrag::Picker { .. }) => Icon::Default,
        Some(UiDrag::Key { moved, .. }) => {
            if moved {
                Icon::EwResize
            } else {
                Icon::Default
            }
        }
        None if pointer.viewport_drag.is_some() => match pointer.viewport_drag {
            Some(viewport::ViewportDrag::Gizmo(_)) => Icon::Grabbing,
            _ => Icon::Default,
        },
        None => controls
            .iter()
            .find(|(i, _)| **i != Interaction::None)
            .and_then(|(_, c)| kind_icon(&c.kind))
            .unwrap_or(if gizmo.0.is_some() {
                Icon::Grab
            } else if target.0.is_some() {
                Icon::Pointer
            } else {
                Icon::Default
            }),
    };
    let wanted = CursorIcon::System(icon);
    for (window, current) in &windows {
        if current != Some(&wanted) {
            commands.entity(window).insert(wanted.clone());
        }
    }
}
