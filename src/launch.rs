//! Launch configuration shared by both entry points and capture tooling.
use bevy::prelude::Resource;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LaunchMode {
    Editor,
    #[default]
    Game,
}

#[derive(Resource)]
pub struct LaunchOptions {
    pub mode: LaunchMode,
    pub workspace: Option<PathBuf>,
    pub world: Option<u32>,
    pub editor_view: Option<String>,
    pub scene: bool,
    pub browser: bool,
    pub mounts: bool,
    pub data_dir: PathBuf,
    pub state_file: Option<PathBuf>,
    pub capture: Option<PathBuf>,
    pub capture_frame: u32,
    pub exit_after_capture: bool,
    pub output_dir: PathBuf,
}

impl Default for LaunchOptions {
    fn default() -> Self {
        Self {
            mode: LaunchMode::Game,
            workspace: Some("test_workspace".into()),
            world: None,
            editor_view: None,
            scene: false,
            browser: false,
            mounts: false,
            data_dir: "data".into(),
            state_file: None,
            capture: None,
            capture_frame: 120,
            exit_after_capture: false,
            output_dir: "artifacts/manual".into(),
        }
    }
}

pub const HELP: &str = "game [--mode editor|game] [--workspace DIR]
  --mode editor|game       Entry point (default: game)
  --workspace DIR          Open this workspace directly (default: test_workspace)
  --world ID               Play a selected world (default: first source path)
  --editor-view VIEW       Open bodies, props, characters, tiles, or worlds in the editor
  --data-dir DIR           Override default workspace; explicit --workspace wins
  --browser | --mounts     Open editor browser or mount controls
  --scene                 Alias for --editor-view characters
  --state-file FILE.json   Write a read-only verification manifest
  --capture FILE.png      Capture renderer output after warmup
  --capture-frame N       Warmup frames (default: 120)
  --exit-after-capture     Exit after capture completes
  --output-dir DIR        F12 screenshot directory (default: artifacts/manual)
  --help, -h              Show this help
Game mode loads the first character in sorted source-file path order.
F12 saves a screenshot in either mode. Ctrl+S saves editor creations.";

impl LaunchOptions {
    pub fn workspace_dir(&self) -> &Path {
        self.workspace.as_deref().unwrap_or(&self.data_dir)
    }

    pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut explicit_workspace = false;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let mut value = || args.next().ok_or_else(|| format!("{arg} requires a value"));
            match arg.as_str() {
                "--world" => {
                    options.world = Some(
                        value()?
                            .parse()
                            .map_err(|_| "--world requires a numeric world ID")?,
                    );
                }
                "--mode" => {
                    options.mode = match value()?.as_str() {
                        "editor" => LaunchMode::Editor,
                        "game" => LaunchMode::Game,
                        other => {
                            return Err(format!("Unknown mode {other}; expected editor or game"));
                        }
                    }
                }
                "--workspace" => {
                    options.workspace = Some(value()?.into());
                    explicit_workspace = true;
                }
                "--editor-view" => {
                    let view = value()?;
                    if !matches!(
                        view.as_str(),
                        "bodies" | "props" | "characters" | "tiles" | "worlds"
                    ) {
                        return Err(format!("Unknown editor view {view}"));
                    }
                    options.editor_view = Some(view);
                }
                "--scene" => options.scene = true,
                "--browser" => options.browser = true,
                "--mounts" => options.mounts = true,
                "--data-dir" => {
                    options.data_dir = value()?.into();
                    if !explicit_workspace {
                        options.workspace = None;
                    }
                }
                "--state-file" => options.state_file = Some(value()?.into()),
                "--capture" => options.capture = Some(value()?.into()),
                "--capture-frame" => {
                    options.capture_frame = value()?
                        .parse()
                        .map_err(|_| "--capture-frame requires a nonnegative integer")?
                }
                "--output-dir" => options.output_dir = value()?.into(),
                "--exit-after-capture" => options.exit_after_capture = true,
                _ => return Err(format!("Unknown argument: {arg}. Use --help.")),
            }
        }
        if options.exit_after_capture && options.capture.is_none() {
            return Err("--exit-after-capture requires --capture".into());
        }
        if options.mode == LaunchMode::Game
            && (options.editor_view.is_some() || options.scene || options.browser || options.mounts)
        {
            return Err("Editor view flags cannot be used with --mode game".into());
        }
        Ok(options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<LaunchOptions, String> {
        LaunchOptions::parse_args(args.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn default_launch_opens_the_sample_game() {
        for args in [vec![], vec!["--mode", "game"]] {
            let options = parse(&args).unwrap();
            assert_eq!(options.mode, LaunchMode::Game);
            assert_eq!(options.workspace_dir(), Path::new("test_workspace"));
        }
        let options = parse(&["--mode", "editor"]).unwrap();
        assert_eq!(options.mode, LaunchMode::Editor);
        assert_eq!(
            options.workspace.as_deref(),
            Some(Path::new("test_workspace"))
        );
    }

    #[test]
    fn modes_and_workspace_paths_are_independent_of_editor_views() {
        for mode in ["editor", "game"] {
            let options = parse(&[
                "--mode",
                mode,
                "--workspace",
                "characters",
                "--data-dir",
                "ignored",
            ])
            .unwrap();
            assert_eq!(options.workspace_dir(), Path::new("characters"));
            assert!(options.editor_view.is_none());
        }
        let options = parse(&[
            "--mode",
            "editor",
            "--editor-view",
            "bodies",
            "--data-dir",
            "isolated",
        ])
        .unwrap();
        assert_eq!(options.workspace_dir(), Path::new("isolated"));
        assert!(options.workspace.is_none());
        assert_eq!(options.editor_view.as_deref(), Some("bodies"));
        let options = parse(&["--data-dir", "isolated", "--workspace", "explicit"]).unwrap();
        assert_eq!(options.workspace_dir(), Path::new("explicit"));
        let options = parse(&["--workspace", "a folder"]).unwrap();
        assert_eq!(options.mode, LaunchMode::Game);
        assert_eq!(options.workspace_dir(), Path::new("a folder"));
    }

    #[test]
    fn invalid_launches_are_reported_without_panics() {
        for args in [
            vec!["--browser"],
            vec!["--mode", "unknown"],
            vec!["--workspace"],
            vec!["--capture-frame", "invalid"],
            vec!["--editor-view", "invalid"],
            vec!["--exit-after-capture"],
            vec!["--mode", "game", "--workspace", "data", "--browser"],
        ] {
            assert!(parse(&args).is_err(), "{args:?}");
        }
    }
}
