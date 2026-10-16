mod editor;
mod launch;
mod play;
mod verification;

use bevy::prelude::*;
use editor::Editor;
use game::body_render::BodyRenderPlugin;
use launch::{LaunchMode, LaunchOptions};
use verification::VerificationPlugin;

fn main() -> bevy::app::AppExit {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", launch::HELP);
        return AppExit::Success;
    }
    let options = match LaunchOptions::parse_args(args) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            return AppExit::from_code(2);
        }
    };
    let mode = options.mode;
    let mut app = App::new();
    match mode {
        LaunchMode::Editor => {
            app.insert_resource(Editor::new(&options));
        }
        LaunchMode::Game => {
            match play::GameSession::load_world(options.workspace_dir(), options.world) {
                Ok(session) => {
                    app.insert_resource(session);
                }
                Err(error) => {
                    eprintln!("Cannot start game: {error}");
                    return AppExit::error();
                }
            }
        }
    }
    app.insert_resource(options)
        .insert_resource(ClearColor(Color::srgb(0.012, 0.014, 0.017)))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/assets").into(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: match mode {
                            LaunchMode::Editor => "Workshop — Game Editor",
                            LaunchMode::Game => "Game | Play Session",
                        }
                        .into(),
                        resolution: bevy::window::WindowResolution::new(1280, 720)
                            .with_scale_factor_override(1.),
                        resizable: true,
                        ..default()
                    }),
                    // The editor asks before discarding unsaved work.
                    close_when_requested: mode == LaunchMode::Game,
                    ..default()
                }),
        )
        .add_plugins((
            VerificationPlugin,
            BodyRenderPlugin,
            game::character_render::CharacterRenderPlugin,
        ));
    match mode {
        LaunchMode::Editor => {
            app.add_plugins(editor::EditorPlugin);
        }
        LaunchMode::Game => {
            app.add_plugins(play::GamePlugin);
        }
    }
    app.run()
}
