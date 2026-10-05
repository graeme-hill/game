//! Validated JSON persistence. Loading never modifies the caller's live library.
use crate::model::{Body, Character, Library, Prop};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
pub fn load(path: &Path) -> Result<Library, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Library::default()),
        Err(e) => return Err(format!("Read {}: {e}", path.display())),
    };
    let library: Library =
        serde_json::from_slice(&bytes).map_err(|e| format!("Parse {}: {e}", path.display()))?;
    library.validate()?;
    Ok(library)
}
pub fn save(path: &Path, library: &Library) -> Result<(), String> {
    library.validate()?;
    let bytes = serde_json::to_vec_pretty(library).map_err(|e| e.to_string())?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let temp = sibling(path, ".tmp");
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        if path.exists() {
            fs::copy(path, sibling(path, ".bak"))?;
        }
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(temp);
        return Err(format!("Save {}: {e}", path.display()));
    }
    Ok(())
}

/// Source files shown by the workspace explorer.  Keeping each asset in a small,
/// explicit file makes a workspace portable and lets tools inspect it without
/// understanding the old aggregate compatibility snapshot.
#[derive(Serialize, Deserialize)]
struct ResourceFile<T> {
    version: u32,
    resource: T,
}

fn read_resource<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let bytes = fs::read(path).map_err(|e| format!("Read {}: {e}", path.display()))?;
    let file: ResourceFile<T> =
        serde_json::from_slice(&bytes).map_err(|e| format!("Parse {}: {e}", path.display()))?;
    if file.version != 1 {
        return Err(format!(
            "Unsupported resource version {} in {}",
            file.version,
            path.display()
        ));
    }
    Ok(file.resource)
}

fn safe_stem(name: &str, id: u32) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim_matches('-');
    format!("{}-{}", if stem.is_empty() { "untitled" } else { stem }, id)
}

fn write_resource<T: Serialize>(
    path: &Path,
    extension: &str,
    name: &str,
    id: u32,
    resource: &T,
) -> Result<(), String> {
    let path = if path.extension().is_some() {
        path.to_path_buf()
    } else {
        path.join(format!("{}.{}", safe_stem(name, id), extension))
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let bytes = serde_json::to_vec_pretty(&ResourceFile {
        version: 1,
        resource,
    })
    .map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn resource_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| format!("Read {}: {e}", dir.display()))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            resource_paths(&path, paths)?;
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("body" | "prop" | "character")
        ) {
            paths.push(path);
        }
    }
    Ok(())
}

fn existing_resource_path(dir: &Path, extension: &str, id: u32) -> Option<PathBuf> {
    let mut paths = vec![];
    resource_paths(dir, &mut paths).ok()?;
    paths.sort();
    paths.into_iter().find(|path| {
        path.extension().and_then(|value| value.to_str()) == Some(extension)
            && fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .and_then(|value| value.get("resource")?.get("id")?.as_u64())
                == Some(u64::from(id))
    })
}

/// Writes the editor's source-defined resources and the legacy aggregate
/// snapshot. Old resource files are intentionally left alone: they may be
/// user-authored files not currently represented by the open library.
pub fn save_workspace(dir: &Path, library: &Library) -> Result<(), String> {
    library.validate()?;
    for body in &library.bodies {
        let path = existing_resource_path(dir, "body", body.id)
            .unwrap_or_else(|| dir.join(format!("{}.body", safe_stem(&body.name, body.id))));
        write_resource(&path, "body", &body.name, body.id, body)?;
    }
    for prop in &library.props {
        let path = existing_resource_path(dir, "prop", prop.id)
            .unwrap_or_else(|| dir.join(format!("{}.prop", safe_stem(&prop.name, prop.id))));
        write_resource(&path, "prop", &prop.name, prop.id, prop)?;
    }
    for character in &library.characters {
        let path = existing_resource_path(dir, "character", character.id).unwrap_or_else(|| {
            dir.join(format!(
                "{}.character",
                safe_stem(&character.name, character.id)
            ))
        });
        write_resource(&path, "character", &character.name, character.id, character)?;
    }
    save(&dir.join("library.json"), library)
}

/// Read source files if present; otherwise open a pre-workspace library.json.
pub fn load_workspace(dir: &Path) -> Result<Library, String> {
    if !dir.is_dir() {
        return load(&dir.join("library.json"));
    }
    // The snapshot is retained for backward compatibility and as an explicit
    // integrity marker. If it exists, never hide a damaged workspace behind
    // otherwise readable source files.
    let snapshot = dir.join("library.json");
    if snapshot.exists() {
        load(&snapshot)?;
    }
    let mut paths = vec![];
    resource_paths(dir, &mut paths)?;
    paths.sort();
    if paths.is_empty() {
        return load(&snapshot);
    }
    let mut library = Library::default();
    for path in paths {
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        match ext {
            "body" => library.bodies.push(read_resource::<Body>(&path)?),
            "prop" => library.props.push(read_resource::<Prop>(&path)?),
            "character" => library.characters.push(read_resource::<Character>(&path)?),
            _ => {}
        }
    }
    library.validate()?;
    Ok(library)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn character_selection_uses_source_path_order_and_checks_versions() {
        let dir = std::env::temp_dir().join(format!("game-order-test-{}", std::process::id()));
        fs::create_dir_all(dir.join("a")).unwrap();
        let body = Body {
            id: 1,
            name: "Body".into(),
            bones: vec![],
            mounts: vec![],
        };
        write_resource(&dir.join("body.body"), "body", "Body", 1, &body).unwrap();
        // Create the later path first and give it a lower ID/name.
        let later = Character {
            id: 2,
            name: "A".into(),
            body: 1,
            attachments: vec![],
            animations: vec![],
        };
        let first = Character {
            id: 3,
            name: "Z".into(),
            body: 1,
            attachments: vec![],
            animations: vec![],
        };
        write_resource(&dir.join("z.character"), "character", "A", 2, &later).unwrap();
        write_resource(&dir.join("a/first.character"), "character", "Z", 3, &first).unwrap();
        assert_eq!(load_workspace(&dir).unwrap().characters, vec![first, later]);
        let path = dir.join("a/first.character");
        let bytes = fs::read_to_string(&path)
            .unwrap()
            .replace("\"version\": 1", "\"version\": 99");
        fs::write(path, bytes).unwrap();
        assert!(
            load_workspace(&dir)
                .unwrap_err()
                .contains("Unsupported resource version 99")
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn legacy_snapshot_is_loaded_when_no_source_files_exist() {
        let dir = std::env::temp_dir().join(format!("game-legacy-test-{}", std::process::id()));
        let library = Library {
            bodies: vec![Body {
                id: 1,
                name: "Legacy".into(),
                bones: vec![],
                mounts: vec![],
            }],
            ..Default::default()
        };
        save(&dir.join("library.json"), &library).unwrap();
        assert_eq!(load_workspace(&dir).unwrap(), library);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn round_trip_backup_and_corrupt_load() {
        let dir = std::env::temp_dir().join(format!("game-storage-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.json");
        assert_eq!(load(&path).unwrap(), Library::default());
        let library = Library {
            bodies: vec![crate::model::Body {
                id: 1,
                name: "My body".into(),
                bones: vec![crate::model::Bone {
                    id: 2,
                    name: "Root".into(),
                    parent: None,
                    offset: [0.; 3],
                    tip: [0., 1., 0.],
                    radius: 0.3,
                    shape: crate::model::BodyShape::Capsule,
                    color: [0.2, 0.6, 0.8],
                }],
                mounts: vec![],
            }],
            ..Default::default()
        };
        save(&path, &library).unwrap();
        assert_eq!(load(&path).unwrap(), library);
        let mut edited = library.clone();
        edited.bodies[0].name = "Renamed body".into();
        save(&path, &edited).unwrap();
        assert_eq!(load(&path).unwrap(), edited);
        assert_eq!(load(&sibling(&path, ".bak")).unwrap(), library);
        fs::write(&path, b"{broken").unwrap();
        assert!(load(&path).is_err());
        let mut invalid = library.clone();
        invalid.version = 2;
        assert!(save(&path, &invalid).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"{broken");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn workspace_resources_round_trip_at_workspace_root() {
        let dir = std::env::temp_dir().join(format!("game-workspace-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let library = Library {
            bodies: vec![crate::model::Body {
                id: 7,
                name: "Hero Body".into(),
                bones: vec![],
                mounts: vec![],
            }],
            ..Default::default()
        };
        save_workspace(&dir, &library).unwrap();
        assert!(dir.join("hero-body-7.body").is_file());
        assert_eq!(load_workspace(&dir).unwrap(), library);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn workspace_discovers_nested_resources_and_saves_in_place() {
        let dir =
            std::env::temp_dir().join(format!("game-nested-workspace-test-{}", std::process::id()));
        let nested = dir.join("characters");
        fs::create_dir_all(&nested).unwrap();
        let library = Library {
            characters: vec![crate::model::Character {
                id: 9,
                name: "Hero".into(),
                body: 1,
                attachments: vec![],
                animations: vec![],
            }],
            bodies: vec![crate::model::Body {
                id: 1,
                name: "Body".into(),
                bones: vec![],
                mounts: vec![],
            }],
            ..Default::default()
        };
        write_resource(
            &nested.join("hero.character"),
            "character",
            "Hero",
            9,
            &library.characters[0],
        )
        .unwrap();
        write_resource(
            &dir.join("body.body"),
            "body",
            "Body",
            1,
            &library.bodies[0],
        )
        .unwrap();
        save(&dir.join("library.json"), &library).unwrap();
        assert_eq!(load_workspace(&dir).unwrap(), library);
        save_workspace(&dir, &library).unwrap();
        assert!(nested.join("hero.character").is_file());
        fs::remove_dir_all(dir).unwrap();
    }
}
