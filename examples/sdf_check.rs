//! Isolated 640x480 renderer diagnostic: front and back voxel geometry, a
//! capsule/sphere smooth union, and automatic capture. See artifacts/sdf-check.
use game::{body_render, model};

use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::WindowResolution,
};
use body_render::{BodyMaterial, BodyRenderPlugin, spawn_body};
use model::{Body, BodyShape, Bone};

fn main() {
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "SDF check".into(),
                        resolution: WindowResolution::new(640, 480).with_scale_factor_override(1.0),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins(BodyRenderPlugin)
        .insert_resource(ClearColor(Color::srgb(0.16, 0.20, 0.27)))
        .add_systems(Startup, setup)
        .add_systems(Update, capture)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BodyMaterial>>,
    mut standard: ResMut<Assets<StandardMaterial>>,
) {
    let body = Body {
        id: 1,
        name: "Diagnostic".into(),
        mounts: vec![],
        bones: vec![
            Bone {
                id: 1,
                name: "Torso".into(),
                parent: None,
                offset: [0.0, -0.6, 0.0],
                tip: [0.0, 1.2, 0.0],
                radius: 0.38,
                shape: BodyShape::Capsule,
                color: [0.15, 0.75, 0.65],
            },
            Bone {
                id: 2,
                name: "Head".into(),
                parent: Some(1),
                offset: [0.0, 0.0, 0.0],
                tip: [0.0, 0.42, 0.0],
                radius: 0.48,
                shape: BodyShape::Sphere,
                color: [0.3, 0.8, 0.65],
            },
            Bone {
                id: 3,
                name: "Arm".into(),
                parent: Some(1),
                offset: [0.0, -0.15, 0.0],
                tip: [0.8, -0.4, 0.0],
                radius: 0.18,
                shape: BodyShape::Capsule,
                color: [0.2, 0.7, 0.9],
            },
        ],
    };
    spawn_body(&mut commands, &mut meshes, &mut materials, &body);
    for (name, position, size, color) in [
        (
            "Front red cube",
            Vec3::new(-0.3, -0.3, 0.4),
            Vec3::splat(0.5),
            Color::srgb(0.9, 0.2, 0.1),
        ),
        (
            "Back gold bar",
            Vec3::new(0.0, 0.2, -0.4),
            Vec3::new(1.7, 0.25, 0.25),
            Color::srgb(0.95, 0.65, 0.1),
        ),
    ] {
        commands.spawn((
            Name::new(name),
            Mesh3d(meshes.add(Cuboid::from_size(size))),
            MeshMaterial3d(standard.add(StandardMaterial {
                base_color: color,
                unlit: true,
                ..default()
            })),
            Transform::from_translation(position),
        ));
    }
    let projection = if std::env::args().any(|arg| arg == "--ortho") {
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: bevy::camera::ScalingMode::FixedVertical {
                viewport_height: 3.8,
            },
            ..OrthographicProjection::default_3d()
        })
    } else {
        Projection::default()
    };
    commands.spawn((
        Camera3d::default(),
        projection,
        Transform::from_xyz(0.0, 0.35, 5.0).looking_at(Vec3::new(0.0, 0.35, 0.0), Vec3::Y),
    ));
}

fn capture(mut commands: Commands, mut frames: Local<u32>) {
    *frames += 1;
    if *frames != 120 {
        return;
    }
    let path = if std::env::args().any(|arg| arg == "--ortho") {
        "artifacts/sdf-check/orthographic.png"
    } else {
        "artifacts/sdf-check/perspective.png"
    };
    commands.spawn(Screenshot::primary_window()).observe(
        move |event: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            std::fs::create_dir_all("artifacts/sdf-check").unwrap();
            event
                .image
                .clone()
                .try_into_dynamic()
                .unwrap()
                .to_rgb8()
                .save(path)
                .unwrap();
            info!("capture_saved path={path}");
            exit.write(AppExit::Success);
        },
    );
}
