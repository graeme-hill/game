//! Capture the actual renderer output; report success only after PNG writing completes.
use crate::launch::LaunchOptions;
use bevy::{
    app::AppExit,
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};

pub struct VerificationPlugin;

impl Plugin for VerificationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, capture);
    }
}

fn capture(
    mut commands: Commands,
    options: Res<LaunchOptions>,
    keys: Res<ButtonInput<KeyCode>>,
    mut frame: Local<u32>,
    mut sequence: Local<u32>,
) {
    *frame += 1;
    let automatic = *frame == options.capture_frame.max(1) && options.capture.is_some();
    if !automatic && !keys.just_pressed(KeyCode::F12) {
        return;
    }
    let path = if automatic {
        options.capture.clone().unwrap()
    } else {
        *sequence += 1;
        options
            .output_dir
            .join(format!("screenshot-{:03}.png", *sequence))
    };
    let exit_after = automatic && options.exit_after_capture;
    info!("capture_requested frame={} path={}", *frame, path.display());
    commands.spawn(Screenshot::primary_window()).observe(
        move |event: On<ScreenshotCaptured>, mut exit: MessageWriter<AppExit>| {
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                event
                    .image
                    .clone()
                    .try_into_dynamic()?
                    .to_rgb8()
                    .save(&path)?;
                Ok(())
            })();
            match result {
                Ok(()) => {
                    info!("capture_saved path={}", path.display());
                    if exit_after {
                        exit.write(AppExit::Success);
                    }
                }
                Err(error) => {
                    error!("capture_failed path={} error={error}", path.display());
                    exit.write(AppExit::error());
                }
            }
        },
    );
}
