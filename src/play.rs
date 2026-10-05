//! Workspace-backed third-person playground. Documents are never mutated in play.
use crate::launch::LaunchOptions;
use bevy::{
    input::mouse::{MouseMotion, MouseWheel},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use game::{
    animation::{blend_clips, forward_kinematics, pose_bounds},
    character_render::{CharacterRenderer, VisualPose},
    controller::*,
    model::Library,
    storage,
};
use std::path::{Path, PathBuf};

#[derive(Resource)]
pub struct GameSession {
    library: Library,
    workspace: PathBuf,
    position: Vec3,
    center: Vec3,
    radius: f32,
    terrain: game::terrain_collision::TerrainCollision,
    height: f32,
}
impl GameSession {
    #[cfg(test)]
    pub fn load(workspace: &Path) -> Result<Self, String> {
        if !workspace.is_dir() {
            return Err(format!(
                "Workspace is not a directory: {}",
                workspace.display()
            ));
        }
        Self::load_world(workspace, None)
    }
    pub fn load_world(workspace: &Path, world: Option<u32>) -> Result<Self, String> {
        if !workspace.is_dir() {
            return Err(format!(
                "Workspace is not a directory: {}",
                workspace.display()
            ));
        }
        let mut library = storage::load_workspace(workspace)?;
        if let Some(id) = world {
            let n = library
                .worlds
                .iter()
                .position(|w| w.id == id)
                .ok_or_else(|| format!("Missing world {id}"))?;
            library.worlds.swap(0, n);
        }
        let character = library
            .characters
            .first()
            .ok_or_else(|| format!("No characters found in {}", workspace.display()))?;
        let body = library
            .bodies
            .iter()
            .find(|b| b.id == character.body)
            .ok_or("Missing body")?;
        let (lower, upper) = body
            .bounds()
            .ok_or_else(|| format!("Character {} has an empty body", character.name))?;
        let midpoint = (lower + upper) * 0.5;
        let position = Vec3::new(-midpoint.x, -lower.y, -midpoint.z);
        let center = midpoint + position;
        let radius = ((upper - lower).length() * 0.5 + 0.2).max(0.5);
        let height = (upper.y - lower.y).max(0.5);
        let terrain = if let Some(w) = library.worlds.first() {
            game::tiles::validate_world(w, &library.tiles, &library.socket_rules, true)?;
            game::terrain_collision::TerrainCollision::from_world(w, &library.tiles)?
        } else {
            Default::default()
        };
        Ok(Self {
            terrain,
            height,
            library,
            workspace: workspace.into(),
            position,
            center,
            radius,
        })
    }
}
#[derive(Component)]
struct PlayerCharacter;
#[derive(Component)]
struct GameCamera;
#[derive(Component)]
struct GameHud;
#[derive(Component)]
struct ContactShadow;

pub struct GamePlugin;
impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            brightness: 400.,
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb(0.16, 0.21, 0.29)))
        .init_resource::<PlayerInput>()
        .init_resource::<FollowCamera>()
        .add_systems(Startup, (spawn_character, setup_environment, spawn_world))
        .add_systems(
            Update,
            (
                read_input,
                move_player,
                animate_player,
                follow_camera,
                update_hud,
            )
                .chain(),
        )
        .add_systems(PostUpdate, snapshot);
    }
}
fn spawn_character(
    session: Res<GameSession>,
    mut renderer: CharacterRenderer,
    mut camera: ResMut<FollowCamera>,
) {
    let character = &session.library.characters[0];
    let parts = renderer.character(&session.library, character, session.position);
    for &entity in &parts {
        renderer.commands.entity(entity).insert(PlayerCharacter);
    }
    renderer.commands.entity(parts[0]).insert(PlayerController {
        position: session
            .library
            .worlds
            .first()
            .map_or(Vec3::ZERO, |w| Vec3::from_array(w.spawn)),
        ..default()
    });
    camera.target = session.center
        + session
            .library
            .worlds
            .first()
            .map_or(Vec3::ZERO, |w| Vec3::from_array(w.spawn));
    camera.distance = (session.radius * 3.8).max(5.);
    if let Some(world) = session.library.worlds.first() {
        camera.yaw = world.spawn_yaw;
    }
    info!(
        "game_character_spawned id={} name={} workspace={}",
        character.id,
        character.name,
        session.workspace.display()
    );
}
fn spawn_world(session: Res<GameSession>, mut renderer: CharacterRenderer) {
    if let Some(world) = session.library.worlds.first() {
        for i in &world.instances {
            if let Ok(t) = game::tiles::tile(&session.library.tiles, i.tile) {
                renderer.tile(t, i.transform());
            }
        }
    }
}
fn setup_environment(
    session: Res<GameSession>,
    mut commands: Commands,
    assets: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if session.library.worlds.is_empty() {
        commands.spawn((
            Name::new("Ground plane"),
            Mesh3d(meshes.add(Plane3d::default().mesh().size(200., 200.))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(0.29, 0.35, 0.40),
                perceptual_roughness: 1.,
                ..default()
            })),
        ));
        // Low-contrast paving gives movement and camera orbit a readable reference.
        let line_material = materials.add(StandardMaterial {
            base_color: Color::srgb(0.25, 0.31, 0.36),
            unlit: true,
            ..default()
        });
        let line_mesh = meshes.add(Cuboid::new(0.025, 0.004, 200.));
        for index in -25..=25 {
            for rotated in [false, true] {
                commands.spawn((
                    Mesh3d(line_mesh.clone()),
                    MeshMaterial3d(line_material.clone()),
                    if rotated {
                        Transform::from_xyz(0., 0.003, index as f32 * 4.)
                            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
                    } else {
                        Transform::from_xyz(index as f32 * 4., 0.003, 0.)
                    },
                ));
            }
        }
    }
    commands.spawn((
        ContactShadow,
        Mesh3d(meshes.add(Circle::new(0.65))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgba(0.03, 0.07, 0.09, 0.22),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..default()
        })),
        Transform::from_xyz(0., 0.015, 0.)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8000.,
            ..default()
        },
        Transform::from_xyz(-3., 7., 5.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((Camera3d::default(), GameCamera, Transform::default()));
    commands.spawn((
        GameHud,
        Text::new(""),
        TextFont {
            font: FontSource::Handle(assets.load("fonts/DejaVuSans.ttf")),
            font_size: FontSize::Px(15.),
            ..default()
        },
        TextColor(Color::srgb(0.94, 0.96, 0.98)),
        Node {
            position_type: PositionType::Absolute,
            left: px(18),
            right: px(18),
            bottom: px(16),
            ..default()
        },
    ));
}

#[derive(bevy::ecs::system::SystemParam)]
struct InputDevices<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    motion: MessageReader<'w, 's, MouseMotion>,
    wheel: MessageReader<'w, 's, MouseWheel>,
    gamepads: Query<'w, 's, &'static Gamepad>,
    window: Single<'w, 's, (&'static Window, &'static mut CursorOptions)>,
}
fn read_input(
    mut devices: InputDevices,
    mut input: ResMut<PlayerInput>,
    mut camera: ResMut<FollowCamera>,
    time: Res<Time>,
) {
    let delta: Vec2 = devices.motion.read().map(|event| event.delta).sum();
    let zoom: f32 = devices.wheel.read().map(|event| event.y).sum();
    let (window, cursor) = &mut *devices.window;
    if !window.focused || devices.keys.just_pressed(KeyCode::Escape) {
        camera.captured = false;
    }
    let start = devices
        .gamepads
        .iter()
        .any(|pad| pad.just_pressed(GamepadButton::Start));
    let capturing = window.focused
        && !camera.captured
        && (devices.mouse.just_pressed(MouseButton::Left) || start);
    if window.focused && start && camera.captured {
        camera.captured = false;
    }
    if capturing {
        camera.captured = true;
    }
    cursor.grab_mode = if camera.captured {
        CursorGrabMode::Locked
    } else {
        CursorGrabMode::None
    };
    cursor.visible = !camera.captured;
    *input = PlayerInput::default();
    if !window.focused || !camera.captured {
        return;
    }
    let axis = |positive, negative| {
        f32::from(devices.keys.pressed(positive)) - f32::from(devices.keys.pressed(negative))
    };
    input.movement = Vec2::new(
        axis(KeyCode::KeyD, KeyCode::KeyA),
        axis(KeyCode::KeyW, KeyCode::KeyS),
    );
    input.speed =
        if devices.keys.pressed(KeyCode::ShiftLeft) || devices.keys.pressed(KeyCode::ShiftRight) {
            RUN_SPEED
        } else {
            WALK_SPEED
        };
    input.jump = devices.keys.just_pressed(KeyCode::Space);
    input.recenter = devices.keys.just_pressed(KeyCode::KeyR);
    input.look = if capturing { Vec2::ZERO } else { delta * 0.003 };
    input.zoom = zoom;
    for pad in &devices.gamepads {
        let movement = deadzone(pad.left_stick());
        if movement.length_squared() > input.movement.length_squared() {
            input.movement = movement;
            input.speed = RUN_SPEED;
        }
        input.look += deadzone(pad.right_stick()) * Vec2::new(2.6, -2.1) * time.delta_secs();
        input.jump |= pad.just_pressed(GamepadButton::South);
        input.recenter |= pad.just_pressed(GamepadButton::RightThumb);
        if pad.pressed(GamepadButton::East) && movement.length_squared() > 0.01 {
            input.movement = movement.normalize();
            input.speed = RUN_SPEED;
        }
    }
}
fn move_player(
    session: Res<GameSession>,
    input: Res<PlayerInput>,
    mut camera: ResMut<FollowCamera>,
    time: Res<Time>,
    mut players: Query<&mut PlayerController>,
) {
    camera.yaw -= input.look.x;
    camera.pitch = (camera.pitch + input.look.y).clamp(-0.10, 1.20);
    camera.distance = (camera.distance - input.zoom * 0.5).clamp(4., 16.);
    for mut player in &mut players {
        if input.recenter {
            camera.yaw = player.facing;
        }
        if session.library.worlds.is_empty() {
            step_player(&mut player, &input, camera.yaw, time.delta_secs());
        } else {
            game::terrain_collision::step_on_terrain(
                &mut player,
                &input,
                camera.yaw,
                time.delta_secs(),
                &session.terrain,
                session.height,
            );
            if player.position.y < -10. {
                player.position = Vec3::from_array(session.library.worlds[0].spawn);
                player.vertical_speed = 0.;
            }
        }
    }
}
fn animate_player(
    session: Res<GameSession>,
    time: Res<Time>,
    mut players: Query<(&PlayerController, &mut VisualPose)>,
) {
    let character = &session.library.characters[0];
    let body = session
        .library
        .bodies
        .iter()
        .find(|body| body.id == character.body)
        .unwrap();
    for (player, mut pose) in &mut players {
        let mut clips = vec![];
        for (index, name) in ["standing", "idle", "walking", "running"]
            .iter()
            .enumerate()
        {
            if let Some(clip) = character
                .animations
                .iter()
                .find(|clip| clip.name.eq_ignore_ascii_case(name))
            {
                let phase = if index >= 2 {
                    player.phase * clip.duration
                } else {
                    time.elapsed_secs()
                };
                clips.push((clip, phase, player.weights[index]));
            }
        }
        pose.locals = blend_clips(body, &clips);
        let frames = forward_kinematics(body, &pose.locals);
        let lower = pose_bounds(body, &frames).unwrap().0;
        let rotation = Quat::from_rotation_y(player.facing);
        pose.world = Transform::from_translation(
            player.position
                + rotation * Vec3::new(session.position.x, -lower.y, session.position.z),
        )
        .with_rotation(rotation);
    }
}
fn follow_camera(
    session: Res<GameSession>,
    time: Res<Time>,
    window: Single<&Window>,
    player: Single<&PlayerController>,
    mut follow: ResMut<FollowCamera>,
    mut camera: Single<&mut Transform, (With<GameCamera>, Without<ContactShadow>)>,
    mut shadow: Single<&mut Transform, (With<ContactShadow>, Without<GameCamera>)>,
) {
    let target = player.position + session.center;
    follow.target = follow
        .target
        .lerp(target, 1. - (-12. * time.delta_secs()).exp());
    let aspect = window.width() / window.height().max(1.);
    let distance = follow.distance / aspect.clamp(0.4, 1.);
    let offset = Vec3::new(
        follow.yaw.sin() * follow.pitch.cos(),
        follow.pitch.sin(),
        follow.yaw.cos() * follow.pitch.cos(),
    ) * distance;
    let mut position = follow.target + offset;
    position.y = position.y.max(0.3); // The only camera obstacle in this scene is the ground.
    if !session.library.worlds.is_empty() {
        position = session.terrain.camera_position(follow.target, position);
    }
    **camera = Transform::from_translation(position).looking_at(follow.target, Vec3::Y);
    shadow.translation = Vec3::new(
        player.position.x,
        session
            .terrain
            .ground(player.position, player.position.y + 0.01, 0.05)
            .unwrap_or(0.)
            + 0.015,
        player.position.z,
    );
    shadow.scale = Vec3::splat((1. - player.position.y * 0.12).clamp(0.4, 1.));
}
fn update_hud(
    camera: Res<FollowCamera>,
    player: Single<&PlayerController>,
    mut hud: Single<&mut Text, With<GameHud>>,
) {
    let text = if camera.captured {
        format!(
            "WASD move  ·  Shift run  ·  Space jump  ·  Mouse look  ·  R recenter  ·  Esc release\nGamepad: left stick move  ·  right stick look  ·  A jump  ·  B sprint  ·  R3 recenter    |    {:.1} m/s",
            player.velocity.length()
        )
    } else {
        "Controls released — click or Start to resume\nWASD + mouse or gamepad".into()
    };
    if hud.0 != text {
        hud.0 = text;
    }
}
fn snapshot(
    options: Res<LaunchOptions>,
    session: Res<GameSession>,
    window: Single<&Window>,
    player: Single<&PlayerController>,
    camera: Res<FollowCamera>,
    parts: Query<Entity, With<PlayerCharacter>>,
    mut ticks: Local<u32>,
) {
    let Some(path) = &options.state_file else {
        return;
    };
    *ticks += 1;
    if !(*ticks).is_multiple_of(5) {
        return;
    }
    let json = serde_json::json!({"mode":"Game","window":[window.resolution.physical_width(),window.resolution.physical_height()],
        "world":session.library.worlds.first(),"collision_boxes":session.terrain.boxes.len(),"workspace":session.workspace,"character":session.library.characters[0],"spawned_parts":parts.iter().len(),
        "position":player.position.to_array(),"speed":player.velocity.length(),"facing":player.facing,"grounded":player.grounded,
        "phase":player.phase,"weights":player.weights,"camera_target":camera.target.to_array(),"camera_yaw":camera.yaw,
        "camera_pitch":camera.pitch,"camera_distance":camera.distance,"captured":camera.captured,"controls":[]});
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tmp = path.with_extension("tmp");
    if let Ok(bytes) = serde_json::to_vec_pretty(&json)
        && std::fs::write(&tmp, bytes).is_ok()
    {
        let _ = std::fs::rename(tmp, path);
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_loads_ten_part_biped_and_places_its_surface_on_the_plane() {
        let session =
            GameSession::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("test_workspace"))
                .unwrap();
        let body = &session.library.bodies[0];
        assert_eq!(body.bones.len(), 10);
        for side in ["Left", "Right"] {
            for limb in ["arm", "leg"] {
                let upper = body
                    .bones
                    .iter()
                    .find(|b| b.name == format!("{side} upper {limb}"))
                    .unwrap();
                let lower = body
                    .bones
                    .iter()
                    .find(|b| b.name == format!("{side} lower {limb}"))
                    .unwrap();
                assert_eq!(lower.parent, Some(upper.id));
                assert_eq!(
                    body.bone_segment(lower.id).unwrap().0,
                    body.bone_segment(upper.id).unwrap().1
                );
            }
        }
        let (lower, upper) = body.bounds().unwrap();
        assert!((lower.y + session.position.y).abs() < 0.0001);
        assert!(((lower + upper) * 0.5 + session.position - session.center).length() < 0.0001);
    }

    #[test]
    fn missing_empty_and_broken_workspaces_fail_before_rendering() {
        let directory = std::env::temp_dir().join(format!("game-play-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        assert!(
            GameSession::load(&directory)
                .err()
                .unwrap()
                .contains("not a directory")
        );
        std::fs::create_dir_all(&directory).unwrap();
        assert!(
            GameSession::load(&directory)
                .err()
                .unwrap()
                .contains("No characters")
        );
        std::fs::write(directory.join("broken.character"), "invalid").unwrap();
        assert!(GameSession::load(&directory).is_err());
        std::fs::remove_file(directory.join("broken.character")).unwrap();
        let mut library =
            storage::load_workspace(&Path::new(env!("CARGO_MANIFEST_DIR")).join("test_workspace"))
                .unwrap();
        library.bodies[0].bones.clear();
        library.characters[0].animations.clear();
        storage::save_workspace(&directory, &library).unwrap();
        assert!(
            GameSession::load(&directory)
                .err()
                .unwrap()
                .contains("empty body")
        );
        std::fs::remove_file(directory.join("library.json")).unwrap();
        // Remove the referenced body to exercise validation of real source files.
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("body") {
                std::fs::remove_file(path).unwrap();
            }
        }
        assert!(
            GameSession::load(&directory)
                .err()
                .unwrap()
                .contains("Missing body")
        );
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    #[test]
    fn gamepad_analog_camera_jump_resume_disconnect_and_focus_loss() {
        let mut app = App::new();
        let mut time = Time::<()>::default();
        time.advance_by(std::time::Duration::from_secs_f32(1. / 60.));
        app.insert_resource(time)
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<PlayerInput>()
            .init_resource::<FollowCamera>()
            .add_message::<MouseMotion>()
            .add_message::<MouseWheel>()
            .add_systems(Update, read_input);
        let window = app
            .world_mut()
            .spawn((
                Window {
                    focused: true,
                    ..default()
                },
                CursorOptions::default(),
            ))
            .id();
        let mut pad = Gamepad::default();
        pad.analog_mut().set(GamepadAxis::LeftStickY, 0.75);
        pad.analog_mut().set(GamepadAxis::RightStickX, 0.8);
        pad.digital_mut().press(GamepadButton::South);
        let entity = app.world_mut().spawn(pad).id();
        app.update();
        let input = *app.world().resource::<PlayerInput>();
        assert!(input.jump && input.look.x > 0.);
        let speed = input.movement.length() * input.speed;
        assert!(speed > WALK_SPEED && speed < RUN_SPEED);
        let weights = locomotion_weights(speed, 0.);
        assert!(weights[2] > 0. && weights[3] > 0.);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = false;
        app.update();
        assert_eq!(app.world().resource::<PlayerInput>().movement, Vec2::ZERO);
        assert!(!app.world().resource::<FollowCamera>().captured);
        app.world_mut().get_mut::<Window>(window).unwrap().focused = true;
        app.world_mut()
            .get_mut::<Gamepad>(entity)
            .unwrap()
            .digital_mut()
            .press(GamepadButton::Start);
        app.update();
        assert!(app.world().resource::<FollowCamera>().captured);
        app.world_mut().despawn(entity);
        app.update();
        assert_eq!(app.world().resource::<PlayerInput>().movement, Vec2::ZERO);
        assert!(!app.world().resource::<PlayerInput>().jump);
    }
}
