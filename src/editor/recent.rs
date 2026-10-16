//! Recently opened workspaces, kept in the user's config directory. Failing
//! to read or write the list never affects editing.
use std::path::{Path, PathBuf};

const LIMIT: usize = 8;

fn file() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(config.join("workshop-editor").join("recent.json"))
}

pub fn load() -> Vec<PathBuf> {
    file()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<Vec<PathBuf>>(&bytes).ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|path| path.is_dir())
        .take(LIMIT)
        .collect()
}

/// Move `dir` to the front of the list and save it.
pub fn remember(list: &mut Vec<PathBuf>, dir: &Path) {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    list.retain(|path| *path != dir);
    list.insert(0, dir);
    list.truncate(LIMIT);
    if let Some(path) = file()
        && let Some(parent) = path.parent()
        && std::fs::create_dir_all(parent).is_ok()
        && let Ok(bytes) = serde_json::to_vec_pretty(list)
    {
        let _ = std::fs::write(path, bytes);
    }
}
