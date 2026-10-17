//! Render a saved world, or a gallery of its tiles, to PNG files from fixed
//! overview cameras. A review tool for tile and world authoring; it uses the
//! same tile renderer and lighting as game mode.
//!
//! ```sh
//! cargo run --example world_shot -- WORKSPACE OUT_DIR [--world ID] [--shadows]
//!     [--shot NAME:YAW:PITCH:DISTANCE[:X:Y:Z]]... [--gallery CATEGORY[,CATEGORY]|all]
//! ```
//! Without `--shot`, a world gets an overview and four corner close-ups.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use game::{
    body_render::BodyRenderPlugin,
    character_render::{CharacterRenderPlugin, CharacterRenderer},
    model::Library,
    storage,
    tiles::{Instance, World},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Clone, Debug)]
struct Shot {
    name: String,
    yaw: f32,
    pitch: f32,
    distance: f32,
    target: Vec3,
}

#[derive(Resource)]
struct Job {
    library: Library,
    world: World,
    shots: Vec<Shot>,
    out: PathBuf,
    character: bool,
    shadows: bool,
    pending: Arc<AtomicUsize>,
}

fn parse() -> Result<Job, String> {
    let mut args = std::env::args().skip(1);
    let workspace = PathBuf::from(
        args.next()
            .ok_or("Usage: world_shot WORKSPACE OUT_DIR ...")?,
    );
    let out = PathBuf::from(args.next().ok_or("Missing OUT_DIR")?);
    let library = storage::load_workspace(&workspace)?;
    let mut world_id = None;
    let mut shots = vec![];
    let mut gallery: Option<String> = None;
    let mut shadows = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--world" => {
                world_id = Some(
                    args.next()
                        .ok_or("--world ID")?
                        .parse::<u32>()
                        .map_err(|e| e.to_string())?,
                )
            }
            "--shadows" => shadows = true,
            "--gallery" => gallery = Some(args.next().ok_or("--gallery CATEGORY")?),
            "--shot" => {
                let spec = args
                    .next()
                    .ok_or("--shot NAME:YAW:PITCH:DISTANCE[:X:Y:Z]")?;
                let parts: Vec<&str> = spec.split(':').collect();
                let num = |i: usize| -> Result<f32, String> {
                    parts
                        .get(i)
                        .ok_or(format!("Bad shot {spec}"))?
                        .parse::<f32>()
                        .map_err(|e| e.to_string())
                };
                shots.push(Shot {
                    name: parts[0].into(),
                    yaw: num(1)?,
                    pitch: num(2)?,
                    distance: num(3)?,
                    target: if parts.len() >= 7 {
                        Vec3::new(num(4)?, num(5)?, num(6)?)
                    } else {
                        Vec3::NAN
                    },
                });
            }
            other => return Err(format!("Unknown argument {other}")),
        }
    }
    if let Some(categories) = gallery {
        // Lay every chosen tile out in a row, far apart, and frame each one.
        let wanted: Vec<&str> = categories.split(',').collect();
        let mut world = World::empty(u32::MAX - 1);
        let mut x = 0.;
        let mut shots_out = vec![];
        for t in library
            .tiles
            .iter()
            .filter(|t| wanted.contains(&"all") || wanted.contains(&t.category.as_str()))
        {
            let ext = t.occupied().iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(lo, hi), b| {
                    (
                        lo.min(Vec3::from_array(b.min)),
                        hi.max(Vec3::from_array(b.max)),
                    )
                },
            );
            if !ext.0.is_finite() {
                continue;
            }
            let size = (ext.1 - ext.0).max_element().max(0.6);
            let centre = (ext.0 + ext.1) * 0.5;
            world.instances.push(Instance {
                id: world.instances.len() as u32 + 1,
                tile: t.id,
                position: [x, 0., 0.],
                rotation: [0., 0., 0., 1.],
            });
            let (yaw, pitch) = shots
                .first()
                .map_or((0.65, 0.62), |s: &Shot| (s.yaw, s.pitch));
            shots_out.push(Shot {
                name: format!(
                    "tile-{}",
                    t.name.to_lowercase().replace([' ', '(', ')'], "-")
                ),
                yaw,
                pitch,
                distance: size * 1.9 + 1.,
                target: centre + Vec3::new(x, 0., 0.),
            });
            x += 60.;
        }
        return Ok(Job {
            world,
            library,
            shots: shots_out,
            out,
            character: false,
            shadows,
            pending: Arc::new(AtomicUsize::new(0)),
        });
    }
    let world = match world_id {
        Some(id) => library.worlds.iter().find(|w| w.id == id),
        None => library.worlds.first(),
    }
    .cloned()
    .ok_or("No world to render")?;
    if shots.is_empty() {
        let size = Vec3::from_array(world.bounds.max) - Vec3::from_array(world.bounds.min);
        let span = size.x.max(size.z);
        shots.push(Shot {
            name: "overview".into(),
            yaw: 0.78,
            pitch: 0.92,
            distance: span * 1.25,
            target: Vec3::NAN,
        });
        for (n, (fx, fz)) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
            .into_iter()
            .enumerate()
        {
            shots.push(Shot {
                name: format!("quarter-{n}"),
                yaw: 0.78,
                pitch: 0.85,
                distance: span * 0.55,
                target: Vec3::new(
                    world.bounds.min[0] + size.x * fx,
                    0.,
                    world.bounds.min[2] + size.z * fz,
                ),
            });
        }
    }
    let centre = (Vec3::from_array(world.bounds.min) + Vec3::from_array(world.bounds.max)) * 0.5;
    for s in &mut shots {
        if !s.target.is_finite() {
            s.target = Vec3::new(centre.x, 0., centre.z);
        }
    }
    Ok(Job {
        library,
        world,
        shots,
        out,
        character: true,
        shadows,
        pending: Arc::new(AtomicUsize::new(0)),
    })
}

fn main() -> AppExit {
    let job = match parse() {
        Ok(job) => job,
        Err(e) => {
            eprintln!("{e}");
            return AppExit::error();
        }
    };
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "World shot".into(),
                        resolution: bevy::window::WindowResolution::new(1280, 720)
                            .with_scale_factor_override(1.),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_plugins((BodyRenderPlugin, CharacterRenderPlugin))
        .insert_resource(GlobalAmbientLight {
            brightness: 400.,
            ..default()
        })
        .insert_resource(ClearColor(Color::srgb(0.16, 0.21, 0.29)))
        .insert_resource(job)
        .add_systems(Startup, setup)
        .add_systems(Update, step)
        .run()
}

fn setup(job: Res<Job>, mut renderer: CharacterRenderer) {
    for i in &job.world.instances {
        if let Ok(t) = game::tiles::tile(&job.library.tiles, i.tile) {
            renderer.tile(t, i.transform());
        }
    }
    if job.character
        && let Some(c) = job.library.characters.first()
        && let Some(body) = job.library.bodies.iter().find(|b| b.id == c.body)
        && let Some((lo, _)) = body.bounds()
    {
        let spawn = Vec3::from_array(job.world.spawn) - Vec3::Y * lo.y;
        renderer.character(&job.library, c, spawn);
    }
    renderer.commands.spawn((
        DirectionalLight {
            illuminance: 8000.,
            shadow_maps_enabled: job.shadows,
            ..default()
        },
        bevy::light::CascadeShadowConfigBuilder {
            first_cascade_far_bound: 20.,
            maximum_distance: 400.,
            ..default()
        }
        .build(),
        Transform::from_xyz(-3., 7., 5.).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    renderer
        .commands
        .spawn((Camera3d::default(), Transform::default()));
}

const WARMUP: u32 = 40;
const GAP: u32 = 12;

fn step(
    mut commands: Commands,
    job: Res<Job>,
    mut frame: Local<u32>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
    mut exit: MessageWriter<AppExit>,
) {
    *frame += 1;
    let f = *frame;
    let index = (f.saturating_sub(WARMUP) / GAP) as usize;
    if f >= WARMUP && index < job.shots.len() {
        let s = &job.shots[index];
        let offset = Vec3::new(
            s.yaw.sin() * s.pitch.cos(),
            s.pitch.sin(),
            s.yaw.cos() * s.pitch.cos(),
        ) * s.distance;
        **camera = Transform::from_translation(s.target + offset).looking_at(s.target, Vec3::Y);
        if (f - WARMUP) % GAP == GAP - 1 {
            let path = job.out.join(format!("{}.png", s.name));
            let pending = job.pending.clone();
            pending.fetch_add(1, Ordering::SeqCst);
            commands.spawn(Screenshot::primary_window()).observe(
                move |e: On<ScreenshotCaptured>| {
                    let _ = std::fs::create_dir_all(path.parent().unwrap());
                    match e.image.clone().try_into_dynamic() {
                        Ok(img) => {
                            let _ = img.to_rgb8().save(&path);
                            println!("saved {}", path.display());
                        }
                        Err(err) => eprintln!("capture failed: {err}"),
                    }
                    pending.fetch_sub(1, Ordering::SeqCst);
                },
            );
        }
    }
    if index >= job.shots.len()
        && f > WARMUP + GAP * job.shots.len() as u32 + 10
        && job.pending.load(Ordering::SeqCst) == 0
    {
        exit.write(AppExit::Success);
    }
}
