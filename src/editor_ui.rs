use crate::editor_animation::{AnimationAction as Anim, AnimationTimeLabel};
use crate::{editor::*, launch::LaunchOptions};
use bevy::{input::mouse::MouseWheel, prelude::*};
use game::voxel;

#[derive(Component)]
pub struct EditorUi;
#[derive(Component)]
pub struct BodyScrollPane;
#[derive(Component)]
pub struct ProUiText;
#[derive(Resource)]
pub struct ProUiFont(Handle<Font>);

pub fn load_pro_ui_font(mut commands: Commands, assets: Res<AssetServer>) {
    commands.insert_resource(ProUiFont(assets.load("fonts/DejaVuSans.ttf")));
}

pub fn apply_pro_ui_font(font: Res<ProUiFont>, mut labels: Query<&mut TextFont, Added<ProUiText>>) {
    for mut label in &mut labels {
        label.font = FontSource::Handle(font.0.clone());
    }
}
fn text(s: impl Into<String>, size: f32) -> impl Bundle {
    (
        Text::new(s),
        TextFont {
            font_size: FontSize::Px(size),
            ..default()
        },
        TextColor(Color::srgb(0.90, 0.91, 0.92)),
        ProUiText,
    )
}
pub(crate) fn item(p: &mut ChildSpawnerCommands, s: impl Into<String>) {
    p.spawn(text(s, 14.));
}
pub(crate) fn button(p: &mut ChildSpawnerCommands, label: &str, action: Action, width: f32) {
    p.spawn((
        Button,
        Control {
            action,
            label: label.into(),
        },
        Node {
            width: px(width),
            height: px(28),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.105, 0.115, 0.125)),
        children![text(label, 13.5)],
    ));
}
pub(crate) fn row(p: &mut ChildSpawnerCommands, items: Vec<(&str, Action, f32)>) {
    p.spawn(Node {
        column_gap: px(3),
        height: px(28),
        ..default()
    })
    .with_children(|r| {
        for (label, action, w) in items {
            button(r, label, action, w)
        }
    });
}
fn vectors(p: &mut ChildSpawnerCommands, v: [f32; 3], rotation: bool) {
    let delta = if rotation {
        std::f32::consts::PI / 12.
    } else {
        0.1
    };
    for (axis, value) in v.into_iter().enumerate() {
        p.spawn(Node {
            column_gap: px(3),
            height: px(24),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Node {
                    width: px(110),
                    height: px(24),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![text(
                    format!(
                        "{} {:>6.2}{}",
                        ["X", "Y", "Z"][axis],
                        if rotation { value.to_degrees() } else { value },
                        if rotation { "d" } else { "" }
                    ),
                    13.
                )],
            ));
            button(r, "-", Action::Axis(axis, -delta), 42.);
            button(r, "+", Action::Axis(axis, delta), 42.);
        });
    }
}
pub(crate) fn palette(p: &mut ChildSpawnerCommands, selected: usize) {
    p.spawn(Node {
        height: px(24),
        column_gap: px(3),
        ..default()
    })
    .with_children(|r| {
        for (i, c) in PALETTE.iter().enumerate() {
            r.spawn((
                Button,
                Control {
                    action: Action::Color(i),
                    label: format!("Color {}", i + 1),
                },
                Node {
                    width: px(22),
                    height: px(24),
                    border: UiRect::all(px(if i == selected { 2 } else { 0 })),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BackgroundColor(Color::srgb(c[0], c[1], c[2])),
            ));
        }
    });
}
fn asset_header(p: &mut ChildSpawnerCommands, e: &Editor) {
    let (name, index, len) = match e.mode {
        Mode::Bodies => (
            e.body().map(|b| b.name.as_str()),
            e.body,
            e.library.bodies.len(),
        ),
        Mode::Props => (
            e.prop().map(|p| p.name.as_str()),
            e.prop,
            e.library.props.len(),
        ),
        _ => (
            e.character().map(|c| c.name.as_str()),
            e.character,
            e.library.characters.len(),
        ),
    };
    item(
        p,
        format!(
            "{:?} {}/{}",
            e.mode,
            if len == 0 { 0 } else { index + 1 },
            len
        ),
    );
    row(
        p,
        vec![
            ("<", Action::Previous, 27.),
            (">", Action::Next, 27.),
            ("New", Action::New, 54.),
            ("Rename", Action::Rename(Target::Asset), 86.),
        ],
    );
    item(
        p,
        name.unwrap_or("Create a new asset")
            .chars()
            .take(23)
            .collect::<String>(),
    );
}

fn workspace_explorer(root: &mut ChildSpawnerCommands, e: &Editor) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(5),
            top: px(38),
            bottom: px(30),
            width: px(222),
            padding: UiRect::all(px(7)),
            flex_direction: FlexDirection::Column,
            row_gap: px(5),
            overflow: Overflow::clip_y(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.045, 0.050, 0.058)),
    ))
    .with_children(|p| {
        p.spawn(text("EXPLORER", 14.));
        item(p, format!("⌄ {}", e.workspace_dir.display()));
        p.spawn(Node {
            column_gap: px(4),
            height: px(28),
            ..default()
        })
        .with_children(|bar| {
            button(bar, "New ▾", Action::ToggleNewMenu, 98.);
            button(bar, "Open workspace", Action::ShowWorkspacePicker, 106.);
        });
        if e.explorer_new_menu {
            item(p, "New resource");
            row(
                p,
                vec![
                    ("Body", Action::NewResource(Mode::Bodies), 64.),
                    ("Prop", Action::NewResource(Mode::Props), 64.),
                    ("Character", Action::NewResource(Mode::Characters), 76.),
                ],
            );
        }
        p.spawn((
            BodyScrollPane,
            ScrollPosition::default(),
            Node {
                width: px(208),
                flex_grow: 1.,
                overflow: Overflow::scroll_y(),
                flex_direction: FlexDirection::Column,
                row_gap: px(2),
                ..default()
            },
        ))
        .with_children(|files| {
            for (index, body) in e.library.bodies.iter().enumerate() {
                tree_choice_button(
                    files,
                    &format!("◈ {}.body", body.name),
                    Action::OpenBody(index),
                    198.,
                    e.mode == Mode::Bodies && e.body == index,
                );
            }
            for (index, prop) in e.library.props.iter().enumerate() {
                tree_choice_button(
                    files,
                    &format!("◆ {}.prop", prop.name),
                    Action::OpenProp(index),
                    198.,
                    e.mode == Mode::Props && e.prop == index,
                );
            }
            for (index, character) in e.library.characters.iter().enumerate() {
                tree_choice_button(
                    files,
                    &format!("● {}.character", character.name),
                    Action::OpenCharacter(index),
                    198.,
                    e.mode == Mode::Characters && e.character == index,
                );
            }
            if matches!(e.mode, Mode::Tiles | Mode::Worlds) {
                for (index, t) in e.library.tiles.iter().enumerate() {
                    tree_choice_button(
                        files,
                        &format!("◆ {}.tile", t.name),
                        Action::World(crate::editor_world::WorldAction::OpenTile(index)),
                        198.,
                        e.mode == Mode::Tiles && e.world_tools.tile == index,
                    );
                }
                for (index, w) in e.library.worlds.iter().enumerate() {
                    tree_choice_button(
                        files,
                        &format!("◇ {}.world", w.name),
                        Action::World(crate::editor_world::WorldAction::OpenWorld(index)),
                        198.,
                        e.mode == Mode::Worlds && e.world_tools.world == index,
                    );
                }
            }
            if e.library.bodies.is_empty()
                && e.library.props.is_empty()
                && e.library.characters.is_empty()
            {
                item(files, "No resource files yet.");
            }
            if e.mode == Mode::Characters {
                character_bone_tree(files, e);
            }
        });
    });
}

fn workspace_picker(root: &mut ChildSpawnerCommands, e: &Editor) {
    let mut directories: Vec<_> = std::fs::read_dir(&e.picker_dir)
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
    root.spawn((
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
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
        GlobalZIndex(20),
    ))
    .with_children(|overlay| {
        overlay
            .spawn((
                Node {
                    width: px(650),
                    height: px(480),
                    padding: UiRect::all(px(16)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(9),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.075, 0.082, 0.094)),
            ))
            .with_children(|panel| {
                panel.spawn(text("Open Workspace", 22.));
                item(
                    panel,
                    "Choose the folder that will contain your .body, .prop, and .character files.",
                );
                panel
                    .spawn(Node {
                        height: px(29),
                        column_gap: px(5),
                        ..default()
                    })
                    .with_children(|bar| {
                        if let Some(parent) = e.picker_dir.parent() {
                            button(
                                bar,
                                "Up",
                                Action::BrowseDirectory(parent.to_path_buf()),
                                52.,
                            );
                        }
                        button(
                            bar,
                            "Select this folder",
                            Action::SelectWorkspace(e.picker_dir.clone()),
                            158.,
                        );
                        button(bar, "Enter path…", Action::ChooseWorkspace, 112.);
                    });
                p_picker_path(panel, &e.picker_dir);
                panel
                    .spawn((
                        BodyScrollPane,
                        ScrollPosition::default(),
                        Node {
                            width: px(618),
                            flex_grow: 1.,
                            overflow: Overflow::scroll_y(),
                            flex_direction: FlexDirection::Column,
                            row_gap: px(3),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        if directories.is_empty() {
                            item(list, "No visible subdirectories.");
                        }
                        for path in directories {
                            let name = path.file_name().map_or_else(
                                || path.display().to_string(),
                                |name| format!("▸ {}", name.to_string_lossy()),
                            );
                            tree_choice_button(
                                list,
                                &name,
                                Action::BrowseDirectory(path),
                                604.,
                                false,
                            );
                        }
                    });
                row(panel, vec![("Cancel", Action::CloseWorkspacePicker, 90.)]);
            });
    });
}

fn p_picker_path(p: &mut ChildSpawnerCommands, path: &std::path::Path) {
    p.spawn((
        Node {
            width: px(618),
            min_height: px(28),
            padding: UiRect::horizontal(px(8)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.035, 0.040, 0.048)),
        children![text(path.display().to_string(), 13.)],
    ));
}

fn editor_tabs(root: &mut ChildSpawnerCommands, e: &Editor) {
    root.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(235),
            top: px(38),
            right: px(290),
            height: px(29),
            column_gap: px(3),
            overflow: Overflow::clip_x(),
            ..default()
        },
        BackgroundColor(Color::srgb(0.030, 0.034, 0.040)),
    ))
    .with_children(|p| {
        if matches!(e.mode, Mode::Tiles | Mode::Worlds) {
            item(
                p,
                if e.mode == Mode::Tiles {
                    "TILE · editable voxel source"
                } else {
                    "WORLD · connected pieces"
                },
            );
        } else if e.open_documents.is_empty() {
            item(p, "No editor open — select a resource in Explorer");
        }
        for document in &e.open_documents {
            let name = match document.mode {
                Mode::Bodies => e
                    .library
                    .bodies
                    .get(document.index)
                    .map(|v| v.name.as_str()),
                Mode::Props => e.library.props.get(document.index).map(|v| v.name.as_str()),
                Mode::Characters => e
                    .library
                    .characters
                    .get(document.index)
                    .map(|v| v.name.as_str()),
                Mode::Tiles | Mode::Worlds | Mode::Menu => None,
            }
            .unwrap_or("missing");
            let action = match document.mode {
                Mode::Bodies => Action::OpenBody(document.index),
                Mode::Props => Action::OpenProp(document.index),
                Mode::Characters => Action::OpenCharacter(document.index),
                Mode::Tiles | Mode::Worlds | Mode::Menu => continue,
            };
            let active = e.mode == document.mode
                && match document.mode {
                    Mode::Bodies => e.body == document.index,
                    Mode::Props => e.prop == document.index,
                    Mode::Characters => e.character == document.index,
                    Mode::Tiles | Mode::Worlds | Mode::Menu => false,
                };
            p.spawn(Node {
                column_gap: px(1),
                height: px(20),
                ..default()
            })
            .with_children(|tab| {
                choice_button(
                    tab,
                    &format!("{}{}", name, if active && e.dirty { " *" } else { "" }),
                    action,
                    108.,
                    active,
                );
                compact_button(tab, "×", Action::CloseDocument(*document), 22.);
            });
        }
    });
}
pub fn rebuild_ui(
    mut commands: Commands,
    e: Res<Editor>,
    old: Query<Entity, With<EditorUi>>,
    mut last: Local<u64>,
) {
    if *last == e.revision {
        return;
    }
    *last = e.revision;
    for entity in &old {
        commands.entity(entity).despawn();
    }
    commands
        .spawn((
            EditorUi,
            Node {
                width: percent(100),
                height: percent(100),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(5),
                    top: px(5),
                    flex_direction: FlexDirection::Row,
                    column_gap: px(4),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.025, 0.028, 0.033)),
            ))
            .with_children(|p| {
                button(p, "Open Workspace…", Action::ShowWorkspacePicker, 132.);
                button(
                    p,
                    if e.dirty { "Save *" } else { "Save" },
                    Action::Save,
                    60.,
                );
                button(p, "Undo", Action::Undo, 48.);
                button(p, "Redo", Action::Redo, 48.);
                button(p, "Reopen", Action::Reload, 66.);
                button(p,"Tiles",Action::Navigate(Mode::Tiles),50.);
                button(p,"Worlds",Action::Navigate(Mode::Worlds),60.);
            });
            if !e.workspace_ready {
                root.spawn((Node { position_type: PositionType::Absolute, left: px(0), right: px(0), top: px(45), bottom: px(50), flex_direction: FlexDirection::Column, row_gap: px(18), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() }, children![text("OPEN A WORKSPACE", 29.), text("A workspace is a directory containing .body, .prop, and .character resources.", 15.)]))
                    .with_children(|p| {
                        button(p, "Use data directory as workspace", Action::UseWorkspace, 300.);
                        button(p, "Choose directory…", Action::ChooseWorkspace, 300.);
                        item(p, "Type a directory path, or launch with --data-dir DIRECTORY.");
                    });
            } else if e.mode == Mode::Menu {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(0),
                        right: px(0),
                        top: px(45),
                        bottom: px(50),
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(18),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    children![
                        text("CHARACTER WORKSHOP", 29.),
                        text("Build a body. Make a hat. Bring them together.", 15.)
                    ],
                ))
                .with_children(|p| {
                    button(
                        p,
                        "1. Bodies / armatures",
                        Action::Navigate(Mode::Bodies),
                        240.,
                    );
                    button(p, "2. Voxel props", Action::Navigate(Mode::Props), 240.);
                    button(
                        p,
                        "3. Assemble characters",
                        Action::Navigate(Mode::Characters),
                        240.,
                    );
                    item(
                        p,
                        format!(
                            "{} bodies  /  {} props  /  {} characters",
                            e.library.bodies.len(),
                            e.library.props.len(),
                            e.library.characters.len()
                        ),
                    );
                    item(p, "Ctrl+S save   Ctrl+Z undo   Ctrl+Y redo");
                });
            } else {
                workspace_explorer(root, &e);
                editor_tabs(root, &e);
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        right: px(5),
                        top: px(72),
                        width: px(280),
                        bottom: px(30),
                        padding: UiRect::all(px(6)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(3),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.052, 0.057, 0.064)),
                ))
                .with_children(|p| {
                    p.spawn(text("INSPECTOR", 14.));
                    if matches!(e.mode,Mode::Tiles|Mode::Worlds) { crate::editor_world::panel(p,&e); } else if e.mode == Mode::Bodies {
                        body_workspace_sidebar(p, &e);
                    } else {
                        asset_header(p, &e);
                        match e.mode {
                            Mode::Bodies => body_panel(p, &e),
                            Mode::Props => prop_panel(p, &e),
                            Mode::Characters => character_panel(p, &e),
                            _ => {}
                        }
                    }
                });
                root.spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(240),
                    bottom: px(16),
                    column_gap: px(5),
                    ..default()
                })
                .with_children(|p| {
                    button(p, "Frame all", Action::Frame, 74.);
                    button(p, "Orbit ‹", Action::Orbit(-0.3), 70.);
                    button(p, "Orbit ›", Action::Orbit(0.3), 70.);
                    button(p, "Near", Action::Zoom(-0.5), 48.);
                    button(p, "Far", Action::Zoom(0.5), 44.);
                    button(
                        p,
                        if e.guides { "Guides on" } else { "Guides off" },
                        Action::Guides,
                        88.,
                    );
                    p.spawn(text(
                        "Right drag orbit  ·  middle drag pan  ·  wheel zoom",
                        11.,
                    ));
                });
            }
            root.spawn((
                text(e.notice.chars().take(100).collect::<String>(), 12.),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(240),
                    bottom: px(7),
                    ..default()
                },
            ));
            if let Some((target, buffer)) = &e.naming {
                root.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(45),
                        top: px(145),
                        width: px(550),
                        height: px(166),
                        padding: UiRect::all(px(18)),
                        flex_direction: FlexDirection::Column,
                        row_gap: px(16),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.10, 0.11, 0.12)),
                    GlobalZIndex(10),
                ))
                .with_children(|p| {
                    item(
                        p,
                        if *target == Target::Workspace {
                            "Workspace directory (Ctrl+A clears; Enter opens)"
                        } else {
                            "Name (Ctrl+A clears; Enter confirms)"
                        },
                    );
                    p.spawn(text(format!("{buffer}_"), 22.));
                    row(
                        p,
                        vec![
                            ("Confirm", Action::NameCommit, 120.),
                            ("Cancel", Action::NameCancel, 120.),
                        ],
                    );
                });
            }
            if e.workspace_picker {
                workspace_picker(root, &e);
            }
        });
}
fn body_panel(p: &mut ChildSpawnerCommands, e: &Editor) {
    row(
        p,
        vec![
            ("Bones", Action::BodyTools(false), 98.),
            ("Mounts", Action::BodyTools(true), 98.),
        ],
    );
    let Some(body) = e.body() else {
        item(p, "Start empty, or use a starter.");
        button(p, "Default skeleton", Action::DefaultSkeleton, 200.);
        button(p, "New starter body", Action::Starter, 200.);
        return;
    };
    if !e.body_mounts {
        body_split_editor(p, e, body);
        return;
    }
    if e.body_mounts {
        item(
            p,
            body.mounts.get(e.mount).map_or("No mounts".into(), |m| {
                format!("{}: {}", e.mount + 1, m.name)
            }),
        );
        row(
            p,
            vec![
                ("<", Action::MountCycle(-1), 27.),
                (">", Action::MountCycle(1), 27.),
                ("Add mount", Action::AddMount, 140.),
            ],
        );
        row(
            p,
            vec![
                ("Delete mount", Action::DeleteMount, 98.),
                ("Name mount", Action::Rename(Target::Mount), 98.),
            ],
        );
        item(
            p,
            body.bones
                .get(e.bone)
                .map_or("Select a bone first".into(), |b| {
                    format!("New mount on: {}", b.name)
                }),
        );
        row(
            p,
            vec![
                ("Position", Action::SetVector(VectorField::MountOffset), 98.),
                (
                    "Rotation",
                    Action::SetVector(VectorField::MountRotation),
                    98.,
                ),
            ],
        );
        if let Some(m) = body.mounts.get(e.mount) {
            vectors(
                p,
                if e.vector == VectorField::MountRotation {
                    m.rotation
                } else {
                    m.offset
                },
                e.vector == VectorField::MountRotation,
            );
        }
        item(p, "Position is relative to bone tip.");
    } else {
        item(
            p,
            body.bones.get(e.bone).map_or("Empty armature".into(), |b| {
                format!("{}/{} {}", e.bone + 1, body.bones.len(), b.name)
            }),
        );
        row(
            p,
            vec![
                ("<", Action::BoneCycle(-1), 25.),
                (">", Action::BoneCycle(1), 25.),
                ("+ Root", Action::AddBone(false), 66.),
                ("+ Child", Action::AddBone(true), 75.),
            ],
        );
        row(
            p,
            vec![
                ("Delete branch", Action::DeleteBone, 104.),
                ("Name bone", Action::Rename(Target::Bone), 92.),
            ],
        );
        row(
            p,
            vec![
                ("Tip XYZ", Action::SetVector(VectorField::BoneTip), 98.),
                ("Joint XYZ", Action::SetVector(VectorField::BoneOffset), 98.),
            ],
        );
        if let Some(b) = body.bones.get(e.bone) {
            vectors(
                p,
                if e.vector == VectorField::BoneOffset {
                    b.offset
                } else {
                    b.tip
                },
                false,
            );
            row(
                p,
                vec![
                    ("Thin -", Action::Radius(-0.03), 61.),
                    (&format!("{:.2}", b.radius), Action::Radius(0.), 67.),
                    ("Thick +", Action::Radius(0.03), 65.),
                ],
            );
            button(p, &format!("Shape: {:?}", b.shape), Action::Shape, 200.);
        }
        palette(p, e.palette);
        if body.bones.is_empty() {
            button(p, "Default skeleton", Action::DefaultSkeleton, 200.);
        }
        button(p, "New starter body", Action::Starter, 200.);
    }
}

fn body_split_editor(p: &mut ChildSpawnerCommands, e: &Editor, body: &game::model::Body) {
    p.spawn((
        BodyScrollPane,
        ScrollPosition::default(),
        Node {
            width: px(268),
            height: px(220),
            overflow: Overflow::scroll_y(),
            flex_direction: FlexDirection::Column,
            row_gap: px(0),
            ..default()
        },
    ))
    .with_children(|props| {
        let Some(bone) = body.bones.get(e.bone) else {
            item(props, "Select a bone above.");
            return;
        };
        props.spawn((
            Node {
                height: px(28),
                padding: UiRect::horizontal(px(8)),
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(Color::srgb(0.075, 0.080, 0.090)),
            children![text(bone.name.clone(), 15.)],
        ));
        item(props, "Position");
        pfield_tabs(props, e.vector);
        compact_vectors(
            props,
            if e.vector == VectorField::BoneOffset {
                bone.offset
            } else {
                bone.tip
            },
        );
        item(props, "Shape");
        button(props, &format!("< {:?} >", bone.shape), Action::Shape, 258.);
        item(props, "Radius");
        row(
            props,
            vec![
                ("−", Action::Radius(-0.03), 78.),
                (&format!("{:.2}", bone.radius), Action::Radius(0.), 96.),
                ("+", Action::Radius(0.03), 78.),
            ],
        );
        item(props, "Material");
        compact_palette(props, e.palette);
    });
}

fn compact_palette(p: &mut ChildSpawnerCommands, selected: usize) {
    p.spawn(Node {
        height: px(22),
        column_gap: px(3),
        ..default()
    })
    .with_children(|row| {
        for (index, color) in PALETTE.iter().enumerate() {
            row.spawn((
                Button,
                Control {
                    action: Action::Color(index),
                    label: format!("Color {}", index + 1),
                },
                Node {
                    width: px(24),
                    height: px(22),
                    border: UiRect::all(px(if index == selected { 2 } else { 0 })),
                    ..default()
                },
                BorderColor::all(Color::WHITE),
                BackgroundColor(Color::srgb(color[0], color[1], color[2])),
            ));
        }
    });
}

fn body_workspace_sidebar(p: &mut ChildSpawnerCommands, e: &Editor) {
    row(
        p,
        vec![
            ("All bodies", Action::Navigate(Mode::Bodies), 96.),
            ("New body", Action::New, 84.),
            ("Rename", Action::Rename(Target::Asset), 82.),
        ],
    );
    item(
        p,
        e.body()
            .map_or("Create a body".into(), |body| body.name.clone()),
    );
    p.spawn(Node {
        column_gap: px(3),
        height: px(22),
        ..default()
    })
    .with_children(|tabs| {
        choice_button(
            tabs,
            "Skeleton",
            Action::BodyTools(false),
            132.,
            !e.body_mounts,
        );
        choice_button(tabs, "Mounts", Action::BodyTools(true), 132., e.body_mounts);
    });
    let Some(body) = e.body() else {
        section(p, "BONES");
        item(p, "Start with a root bone.");
        button(p, "Default skeleton", Action::DefaultSkeleton, 268.);
        button(p, "New starter body", Action::Starter, 268.);
        return;
    };
    if e.body_mounts {
        mount_workspace(p, e, body);
    } else {
        section(p, "BONES");
        if body.bones.is_empty() {
            item(p, "Start from a humanoid template or add a root bone.");
            button(p, "Default skeleton", Action::DefaultSkeleton, 268.);
        }
        body_outline_list(p, body, e.bone, 148.);
        row(
            p,
            vec![
                ("‹", Action::BoneCycle(-1), 32.),
                ("›", Action::BoneCycle(1), 32.),
                ("Add root", Action::AddBone(false), 96.),
                ("Add child", Action::AddBone(true), 100.),
            ],
        );
        row(
            p,
            vec![
                ("Delete branch", Action::DeleteBone, 128.),
                ("Rename", Action::Rename(Target::Bone), 128.),
            ],
        );
        section(p, "PROPERTIES");
        body_split_editor(p, e, body);
    }
}

fn mount_workspace(p: &mut ChildSpawnerCommands, e: &Editor, body: &game::model::Body) {
    section(p, "BONE FOR NEW MOUNTS");
    body_outline_list(p, body, e.bone, 92.);
    section(p, "MOUNTS");
    mount_outline_list(p, body, e.mount, 92.);
    row(
        p,
        vec![("Add mount to selected bone", Action::AddMount, 268.)],
    );
    row(
        p,
        vec![
            ("Delete mount", Action::DeleteMount, 128.),
            ("Rename", Action::Rename(Target::Mount), 128.),
        ],
    );
    section(p, "MOUNT PROPERTIES");
    if let Some(mount) = body.mounts.get(e.mount) {
        item(
            p,
            format!(
                "{}  ·  parent: {}",
                mount.name,
                body.bones
                    .iter()
                    .find(|bone| bone.id == mount.bone)
                    .map_or("missing", |bone| bone.name.as_str())
            ),
        );
        p.spawn(Node {
            column_gap: px(3),
            height: px(22),
            ..default()
        })
        .with_children(|tabs| {
            choice_button(
                tabs,
                "Offset",
                Action::SetVector(VectorField::MountOffset),
                132.,
                e.vector == VectorField::MountOffset,
            );
            choice_button(
                tabs,
                "Rotation",
                Action::SetVector(VectorField::MountRotation),
                132.,
                e.vector == VectorField::MountRotation,
            );
        });
        vectors(
            p,
            if e.vector == VectorField::MountRotation {
                mount.rotation
            } else {
                mount.offset
            },
            e.vector == VectorField::MountRotation,
        );
    } else {
        item(p, "Select a bone, then add a mount.");
    }
}

fn mount_outline_list(
    p: &mut ChildSpawnerCommands,
    body: &game::model::Body,
    selected: usize,
    height: f32,
) {
    p.spawn((
        BodyScrollPane,
        ScrollPosition::default(),
        Node {
            width: px(268),
            height: px(height),
            overflow: Overflow::scroll_y(),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        },
    ))
    .with_children(|list| {
        for (index, mount) in body.mounts.iter().enumerate() {
            tree_choice_button(
                list,
                &format!(
                    "{}  {}",
                    if index == selected { "●" } else { "○" },
                    mount.name
                ),
                Action::SelectMount(index),
                258.,
                index == selected,
            );
        }
        if body.mounts.is_empty() {
            item(list, "No mounts yet.");
        }
    });
}

fn body_outline_list(
    p: &mut ChildSpawnerCommands,
    body: &game::model::Body,
    selected: usize,
    height: f32,
) {
    p.spawn((
        BodyScrollPane,
        ScrollPosition::default(),
        Node {
            width: px(268),
            height: px(height),
            overflow: Overflow::scroll_y(),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        },
    ))
    .with_children(|tree| {
        for (index, bone) in body.bones.iter().enumerate() {
            let mut depth = 0;
            let mut parent = bone.parent;
            while let Some(id) = parent {
                depth += 1;
                parent = body
                    .bones
                    .iter()
                    .find(|candidate| candidate.id == id)
                    .and_then(|candidate| candidate.parent);
            }
            let label = format!(
                "{}{}  {}",
                "    ".repeat(depth),
                if index == selected { "●" } else { "○" },
                bone.name
            );
            tree_choice_button(
                tree,
                &label,
                Action::SelectBone(index),
                258.,
                index == selected,
            );
        }
    });
}

fn tree_choice_button(
    p: &mut ChildSpawnerCommands,
    label: &str,
    action: Action,
    width: f32,
    active: bool,
) {
    p.spawn((
        Button,
        Control {
            action,
            label: label.into(),
        },
        Node {
            width: px(width),
            height: px(24),
            padding: UiRect::left(px(9)),
            justify_content: JustifyContent::FlexStart,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(2)),
            ..default()
        },
        BackgroundColor(if active {
            Color::srgb(0.31, 0.33, 0.36)
        } else {
            Color::srgb(0.068, 0.074, 0.082)
        }),
        children![text(label, 13.)],
    ));
}

fn pfield_tabs(p: &mut ChildSpawnerCommands, active: VectorField) {
    p.spawn(Node {
        column_gap: px(3),
        height: px(20),
        ..default()
    })
    .with_children(|tabs| {
        choice_button(
            tabs,
            "Bone end",
            Action::SetVector(VectorField::BoneTip),
            128.,
            active == VectorField::BoneTip,
        );
        choice_button(
            tabs,
            "Joint offset",
            Action::SetVector(VectorField::BoneOffset),
            128.,
            active == VectorField::BoneOffset,
        );
    });
}

fn choice_button(
    p: &mut ChildSpawnerCommands,
    label: &str,
    action: Action,
    width: f32,
    active: bool,
) {
    p.spawn((
        Button,
        Control {
            action,
            label: label.into(),
        },
        Node {
            width: px(width),
            height: px(20),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(if active {
            Color::srgb(0.31, 0.33, 0.36)
        } else {
            Color::srgb(0.085, 0.092, 0.102)
        }),
        children![text(label, 12.)],
    ));
}

fn compact_vectors(p: &mut ChildSpawnerCommands, v: [f32; 3]) {
    for (axis, value) in v.into_iter().enumerate() {
        p.spawn(Node {
            column_gap: px(3),
            height: px(20),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|r| {
            r.spawn((
                Node {
                    width: px(152),
                    height: px(20),
                    align_items: AlignItems::Center,
                    ..default()
                },
                children![text(
                    format!("{} {:>6.2}", ["X", "Y", "Z"][axis], value),
                    11.
                )],
            ));
            compact_button(r, "−", Action::Axis(axis, -0.1), 52.);
            compact_button(r, "+", Action::Axis(axis, 0.1), 52.);
        });
    }
}

fn compact_button(p: &mut ChildSpawnerCommands, label: &str, action: Action, width: f32) {
    p.spawn((
        Button,
        Control {
            action,
            label: label.into(),
        },
        Node {
            width: px(width),
            height: px(20),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: BorderRadius::all(px(3)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.14, 0.15, 0.17)),
        children![text(label, 11.)],
    ));
}

fn section(p: &mut ChildSpawnerCommands, label: &str) {
    p.spawn((
        Node {
            width: px(268),
            height: px(26),
            padding: UiRect::left(px(8)),
            align_items: AlignItems::Center,
            ..default()
        },
        BackgroundColor(Color::srgb(0.13, 0.14, 0.16)),
        children![text(label, 14.)],
    ));
}
fn prop_panel(p: &mut ChildSpawnerCommands, e: &Editor) {
    row(
        p,
        vec![
            ("Voxels", Action::PropTools(false), 98.),
            ("Anchors", Action::PropTools(true), 98.),
        ],
    );
    let Some(prop) = e.prop() else {
        item(p, "New creates an empty 32³ grid.");
        return;
    };
    if e.prop_anchors {
        item(
            p,
            prop.anchors.get(e.anchor).map_or("No anchors".into(), |a| {
                format!("{}: {}", e.anchor + 1, a.name)
            }),
        );
        row(
            p,
            vec![
                ("<", Action::AnchorCycle(-1), 27.),
                (">", Action::AnchorCycle(1), 27.),
                ("Add anchor", Action::AddAnchor, 140.),
            ],
        );
        row(
            p,
            vec![
                ("Delete anchor", Action::DeleteAnchor, 104.),
                ("Name anchor", Action::Rename(Target::Anchor), 92.),
            ],
        );
        row(
            p,
            vec![
                (
                    "Position",
                    Action::SetVector(VectorField::AnchorPosition),
                    98.,
                ),
                (
                    "Rotation",
                    Action::SetVector(VectorField::AnchorRotation),
                    98.,
                ),
            ],
        );
        if let Some(a) = prop.anchors.get(e.anchor) {
            vectors(
                p,
                if e.vector == VectorField::AnchorRotation {
                    a.rotation
                } else {
                    a.position
                },
                e.vector == VectorField::AnchorRotation,
            );
        }
        item(p, "New anchor starts at cursor.");
        item(p, "Match it to a body mount.");
    } else {
        row(
            p,
            vec![
                ("Brush -", Action::Brush(-1), 68.),
                (&format!("{}x", e.brush), Action::Brush(0), 57.),
                ("Brush +", Action::Brush(1), 68.),
            ],
        );
        for axis in 0..3 {
            p.spawn(Node {
                column_gap: px(3),
                height: px(24),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|r| {
                r.spawn((
                    Node {
                        width: px(110),
                        height: px(24),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    children![text(
                        format!("{} {:>2}", ["X", "Y", "Z"][axis], e.cursor[axis]),
                        14.
                    )],
                ));
                button(r, "-", Action::Cursor(axis, -1), 42.);
                button(r, "+", Action::Cursor(axis, 1), 42.);
            });
        }
        row(
            p,
            vec![
                ("Paint cursor", Action::Paint(false), 98.),
                ("Erase cursor", Action::Paint(true), 98.),
            ],
        );
        row(
            p,
            vec![
                (
                    if e.erase { "Click: paint" } else { "Paint ON" },
                    Action::PaintMode(false),
                    98.,
                ),
                (
                    if e.erase { "Erase ON" } else { "Click: erase" },
                    Action::PaintMode(true),
                    98.,
                ),
            ],
        );
        palette(p, e.palette);
        let stats = voxel::mesh_stats(prop);
        item(
            p,
            format!("{} blocks / {} quads", stats.block_count, stats.quad_count),
        );
        item(p, "Click grid or a voxel face.");
        item(p, "Y sets the editing layer.");
    }
}
fn character_panel(p: &mut ChildSpawnerCommands, e: &Editor) {
    row(
        p,
        vec![
            ("Assembly", Action::Animation(Anim::Tools(false)), 98.),
            ("Animations", Action::Animation(Anim::Tools(true)), 108.),
        ],
    );
    if e.animation.enabled {
        animation_panel(p, e);
        return;
    }

    let Some(c) = e.character() else {
        item(p, "Create a body and prop first.");
        return;
    };
    button(
        p,
        &format!(
            "Body: {}",
            e.preview_body().map_or("missing", |b| b.name.as_str())
        ),
        Action::ChooseBody,
        200.,
    );
    button(
        p,
        &format!("Prop: {}", e.prop().map_or("none", |p| p.name.as_str())),
        Action::ChooseProp,
        200.,
    );
    item(
        p,
        e.preview_body()
            .and_then(|b| b.mounts.get(e.mount))
            .map_or("Mount: none".into(), |m| format!("Mount: {}", m.name)),
    );
    row(
        p,
        vec![
            ("Mount <", Action::MountCycle(-1), 98.),
            ("Mount >", Action::MountCycle(1), 98.),
        ],
    );
    item(
        p,
        e.prop()
            .and_then(|p| p.anchors.get(e.anchor))
            .map_or("Anchor: none".into(), |a| format!("Anchor: {}", a.name)),
    );
    row(
        p,
        vec![
            ("Anchor <", Action::AnchorCycle(-1), 98.),
            ("Anchor >", Action::AnchorCycle(1), 98.),
        ],
    );
    row(
        p,
        vec![
            ("Attach prop", Action::Attach, 98.),
            ("Remove", Action::RemoveAttachment, 98.),
        ],
    );
    row(
        p,
        vec![
            ("Item <", Action::AttachmentCycle(-1), 58.),
            (
                &format!(
                    "{}/{}",
                    if c.attachments.is_empty() {
                        0
                    } else {
                        e.attachment + 1
                    },
                    c.attachments.len()
                ),
                Action::AttachmentCycle(0),
                72.,
            ),
            ("Item >", Action::AttachmentCycle(1), 60.),
        ],
    );
    if let Some(a) = c.attachments.get(e.attachment) {
        row(
            p,
            vec![
                (
                    "Position",
                    Action::SetVector(VectorField::AttachmentOffset),
                    98.,
                ),
                (
                    "Rotation",
                    Action::SetVector(VectorField::AttachmentRotation),
                    98.,
                ),
            ],
        );
        vectors(
            p,
            if e.vector == VectorField::AttachmentRotation {
                a.rotation
            } else {
                a.offset
            },
            e.vector == VectorField::AttachmentRotation,
        );
        row(
            p,
            vec![
                ("Scale -", Action::Scale(-0.1), 70.),
                (&format!("{:.1}x", a.scale), Action::Scale(0.), 55.),
                ("Scale +", Action::Scale(0.1), 68.),
            ],
        );
    }
}

fn animation_row(p: &mut ChildSpawnerCommands, items: Vec<(&str, Action, f32)>) {
    p.spawn(Node {
        column_gap: px(3),
        height: px(23),
        flex_shrink: 0.,
        ..default()
    })
    .with_children(|row| {
        for (label, action, width) in items {
            row.spawn((
                Button,
                Control {
                    action,
                    label: label.into(),
                },
                Node {
                    width: px(width),
                    height: px(23),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    border_radius: BorderRadius::all(px(3)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.105, 0.115, 0.125)),
                children![text(label, 12.)],
            ));
        }
    });
}

fn animation_panel(p: &mut ChildSpawnerCommands, e: &Editor) {
    let Some(character) = e.character() else {
        item(p, "Create a character first.");
        return;
    };
    let count = character.animations.len();
    animation_row(
        p,
        vec![
            ("New", Action::Animation(Anim::New), 62.),
            ("Copy", Action::Animation(Anim::Duplicate), 62.),
            ("Delete", Action::Animation(Anim::Delete), 70.),
        ],
    );
    animation_row(
        p,
        vec![
            ("Rename", Action::Rename(Target::Animation), 92.),
            ("Add locomotion", Action::Animation(Anim::Presets), 122.),
        ],
    );
    let Some(clip) = e.animation_clip() else {
        item(p, "Add a clip, or use the locomotion starter.");
        return;
    };
    animation_row(
        p,
        vec![
            (
                "<",
                Action::Animation(Anim::Select((e.animation.selected + count - 1) % count)),
                30.,
            ),
            (
                &format!("{} ({}/{})", clip.name, e.animation.selected + 1, count),
                Action::Animation(Anim::Select(e.animation.selected)),
                158.,
            ),
            (
                ">",
                Action::Animation(Anim::Select((e.animation.selected + 1) % count)),
                30.,
            ),
        ],
    );
    animation_row(
        p,
        vec![
            ("Length -", Action::Animation(Anim::Duration(-0.1)), 70.),
            (
                &format!("{:.1}s", clip.duration),
                Action::Animation(Anim::Duration(0.)),
                60.,
            ),
            ("Length +", Action::Animation(Anim::Duration(0.1)), 70.),
        ],
    );
    animation_row(
        p,
        vec![
            (
                if e.animation.playing { "Pause" } else { "Play" },
                Action::Animation(Anim::Play),
                98.,
            ),
            (
                if clip.looping {
                    "Loop: on"
                } else {
                    "Loop: off"
                },
                Action::Animation(Anim::Loop),
                98.,
            ),
        ],
    );
    p.spawn((
        text(format!("Time {:.2}s", e.animation.time), 13.),
        AnimationTimeLabel,
    ));
    animation_row(
        p,
        vec![
            ("Start", Action::Animation(Anim::Time(0.)), 48.),
            ("-.1s", Action::Animation(Anim::Step(-0.1)), 48.),
            ("+.1s", Action::Animation(Anim::Step(0.1)), 48.),
            ("End", Action::Animation(Anim::Time(clip.duration)), 48.),
        ],
    );
    let bone = e.preview_body().and_then(|body| body.bones.get(e.bone));
    item(
        p,
        format!("Bone: {}", bone.map_or("none", |bone| bone.name.as_str())),
    );
    let keys = bone
        .and_then(|b| clip.tracks.iter().find(|track| track.bone == b.id))
        .map(|track| track.keys.as_slice())
        .unwrap_or_default();
    let previous = keys
        .iter()
        .rev()
        .find(|key| key.time < e.animation.time - 0.001)
        .map_or(0., |key| key.time);
    let next = keys
        .iter()
        .find(|key| key.time > e.animation.time + 0.001)
        .map_or(clip.duration, |key| key.time);
    animation_row(
        p,
        vec![
            ("Key <", Action::Animation(Anim::Time(previous)), 62.),
            ("Key >", Action::Animation(Anim::Time(next)), 62.),
            (
                &format!("{} keys", keys.len()),
                Action::Animation(Anim::Time(e.animation.time)),
                70.,
            ),
        ],
    );
    animation_row(
        p,
        vec![
            ("Rotation", Action::Animation(Anim::Translation(false)), 98.),
            ("Offset", Action::Animation(Anim::Translation(true)), 98.),
        ],
    );
    let pose = e.animation_values();
    let (x, y, z) = pose.rotation.to_euler(EulerRot::XYZ);
    let values = if e.animation.translation {
        pose.translation.to_array()
    } else {
        [x.to_degrees(), y.to_degrees(), z.to_degrees()]
    };
    let delta = if e.animation.translation {
        0.025
    } else {
        5_f32.to_radians()
    };
    for (axis, value) in values.into_iter().enumerate() {
        animation_row(
            p,
            vec![
                (
                    &format!("{} {:>6.2}", ["X", "Y", "Z"][axis], value),
                    Action::Animation(Anim::Adjust(axis, 0.)),
                    104.,
                ),
                ("-", Action::Animation(Anim::Adjust(axis, -delta)), 42.),
                ("+", Action::Animation(Anim::Adjust(axis, delta)), 42.),
            ],
        );
    }
    animation_row(
        p,
        vec![
            ("Set key", Action::Animation(Anim::Key), 98.),
            ("Delete key", Action::Animation(Anim::DeleteKey), 98.),
        ],
    );
    let other = &character.animations[e.animation.blend_target.min(count - 1)];
    animation_row(
        p,
        vec![
            (
                "Blend with >",
                Action::Animation(Anim::SelectBlend((e.animation.blend_target + 1) % count)),
                98.,
            ),
            (
                &other.name,
                Action::Animation(Anim::SelectBlend(e.animation.blend_target)),
                110.,
            ),
        ],
    );
    animation_row(
        p,
        vec![
            ("Blend -", Action::Animation(Anim::Blend(-0.1)), 70.),
            (
                &format!("{:.0}%", e.animation.blend * 100.),
                Action::Animation(Anim::Blend(0.)),
                60.,
            ),
            ("Blend +", Action::Animation(Anim::Blend(0.1)), 70.),
        ],
    );
    p.spawn(text(
        "Select bones at left. +/- edits a key at the cursor.
Rotation in degrees; offsets in body units.",
        11.,
    ));
}

fn character_bone_tree(root: &mut ChildSpawnerCommands, e: &Editor) {
    let Some(body) = e
        .library
        .bodies
        .iter()
        .find(|body| Some(body.id) == e.character().map(|c| c.body))
    else {
        return;
    };
    root.spawn((
        Node {
            width: px(198),
            flex_shrink: 0.,
            padding: UiRect::all(px(5)),
            flex_direction: FlexDirection::Column,
            row_gap: px(2),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.07, 0.10, 0.92)),
    ))
    .with_children(|p| {
        item(p, "BONES — click to select");
        for (index, bone) in body.bones.iter().enumerate() {
            let mut depth = 0;
            let mut parent = bone.parent;
            while let Some(id) = parent {
                depth += 1;
                parent = body
                    .bones
                    .iter()
                    .find(|candidate| candidate.id == id)
                    .and_then(|candidate| candidate.parent);
            }
            let marker = if index == e.bone { ">" } else { " " };
            let label = format!("{}{} {}", "  ".repeat(depth), marker, bone.name);
            p.spawn((
                Button,
                Control {
                    action: Action::SelectBone(index),
                    label: label.clone(),
                },
                Node {
                    width: px(180),
                    height: px(19),
                    padding: UiRect::left(px(3)),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(if index == e.bone {
                    Color::srgb(0.31, 0.33, 0.36)
                } else {
                    Color::srgb(0.085, 0.092, 0.102)
                }),
                children![text(label, 11.)],
            ));
        }
    });
}

pub fn scroll_body_panes(
    mut wheel: MessageReader<MouseWheel>,
    windows: Single<&Window>,
    mut panes: Query<
        (&ComputedNode, &UiGlobalTransform, &mut ScrollPosition),
        With<BodyScrollPane>,
    >,
) {
    let delta: f32 = wheel.read().map(|event| event.y).sum();
    if delta == 0. {
        return;
    }
    let Some(cursor) = windows.cursor_position() else {
        return;
    };
    for (node, transform, mut position) in &mut panes {
        let size = node.size();
        let center = transform.translation;
        let inside = cursor.x >= center.x - size.x * 0.5
            && cursor.x <= center.x + size.x * 0.5
            && cursor.y >= center.y - size.y * 0.5
            && cursor.y <= center.y + size.y * 0.5;
        if inside {
            position.0.y = (position.0.y - delta * 24.).max(0.);
        }
    }
}

pub fn snapshot(
    e: Res<Editor>,
    windows: Single<&Window>,
    options: Res<LaunchOptions>,
    buttons: Query<(
        &Control,
        &ComputedNode,
        &UiGlobalTransform,
        Option<&CalculatedClip>,
    )>,
    mut ticks: Local<u32>,
) {
    let Some(path) = &options.state_file else {
        return;
    };
    *ticks += 1;
    if !(*ticks).is_multiple_of(5) {
        return;
    }
    let controls:Vec<_>=buttons.iter().filter_map(|(c,node,t,clip)|{
        let mut rect=Rect::from_center_size(t.translation,node.size());
        if let Some(clip)=clip {rect=rect.intersect(clip.clip);}
        rect=rect.intersect(Rect::from_corners(Vec2::ZERO,Vec2::new(windows.resolution.physical_width() as f32,windows.resolution.physical_height() as f32)));
        let center=rect.center();let size=rect.size();if size.x<=0.||size.y<=0.{return None}
        Some(serde_json::json!({"label":c.label,"action":format!("{:?}",c.action),"center":[center.x,center.y],"size":[size.x,size.y]}))
    }).collect();
    let state = serde_json::json!({"mode":format!("{:?}",e.mode),"revision":e.revision,"notice":e.notice,"naming":e.naming.as_ref().map(|(_,s)|s),"workspace":e.workspace_dir,"workspace_ready":e.workspace_ready,"open_documents":e.open_documents,"library":e.library,"tile":e.world_tools.tile,"world":e.world_tools.world,"instance":e.world_tools.instance,"socket":e.world_tools.socket,"tile_rotation":e.world_tools.rotation,"body":e.body,"prop":e.prop,"character":e.character,"bone":e.bone,"mount":e.mount,"anchor":e.anchor,"attachment":e.attachment,"cursor":e.cursor,"brush":e.brush,"animation":{"enabled":e.animation.enabled,"selected":e.animation.selected,"time":e.animation.time,"playing":e.animation.playing,"blend":e.animation.blend,"blend_target":e.animation.blend_target},"dirty":e.dirty,"window":[windows.resolution.physical_width(),windows.resolution.physical_height()],"controls":controls});
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if let Ok(bytes) = serde_json::to_vec(&state)
        && std::fs::write(&tmp, bytes).is_ok()
    {
        let _ = std::fs::rename(tmp, path);
    }
}
