use crate::files::{allowed_paths, checked_path, fingerprint};
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Serialize)]
pub struct Item {
    pub id: usize,
    pub section: usize,
    pub path: String,
    pub bytes: u64,
    #[serde(skip)]
    root: PathBuf,
    #[serde(skip)]
    hash: String,
}

#[derive(Clone, Serialize)]
pub struct Preview {
    pub name: String,
    pub appdata: Option<String>,
    pub items: Vec<Item>,
}

// Development must never discover or delete the real playable copies.
pub fn appdata_root(game: &Path) -> Option<PathBuf> {
    if cfg!(debug_assertions) {
        return Some(game.join(".appdata/Activision/CoDWaW/mods"));
    }

    std::env::var_os("LOCALAPPDATA").map(|root| PathBuf::from(root).join("Activision/CoDWaW/mods"))
}

fn children(root: &Path, relative: &str) -> Result<Vec<(String, bool)>, String> {
    check_root(root)?;
    let directory = checked_path(root, relative)?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut result = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "Unsupported filename.")?;
        let child = format!("{relative}/{name}");
        // Reject junctions and symbolic links before walking into a directory.
        let path = checked_path(root, &child)?;
        result.push((child, path.is_dir()));
    }

    result.sort();
    Ok(result)
}

fn check_root(root: &Path) -> Result<(), String> {
    for ancestor in root.ancestors() {
        if let (Some(parent), Some(name)) = (ancestor.parent(), ancestor.file_name()) {
            checked_path(parent, &name.to_string_lossy())?;
        }
    }
    Ok(())
}

fn walk(root: &Path, relative: &str, result: &mut BTreeSet<String>) -> Result<(), String> {
    for (path, directory) in children(root, relative)? {
        if directory {
            walk(root, &path, result)?;
        } else {
            result.insert(path);
        }
    }
    Ok(())
}

// Discover incomplete maps too, including those whose starter files were deleted.
pub fn maps(game: &Path, appdata: Option<&Path>) -> Result<Vec<String>, String> {
    let mut names = BTreeSet::new();
    for (path, directory) in children(game, "mods")? {
        if directory {
            names.insert(path.trim_start_matches("mods/").to_string());
        }
    }

    for (path, directory) in children(game, "map_source")? {
        if !directory && path.ends_with(".map") {
            let name = path
                .trim_start_matches("map_source/")
                .trim_end_matches(".map");
            if !name.ends_with("_patch") {
                names.insert(name.to_string());
            }
        }
    }

    if let Some(root) = appdata {
        if root.exists() {
            // Validate the AppData mods directory itself before enumerating it.
            let parent = root.parent().ok_or("Invalid AppData folder.")?;
            for (path, directory) in children(parent, "mods")? {
                if directory {
                    names.insert(path.trim_start_matches("mods/").to_string());
                }
            }
        }
    }

    for (path, directory) in children(game, "raw/maps")? {
        if !directory {
            let path = Path::new(&path);
            if matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("d3dbsp" | "radtrans" | "grid_auto" | "d3dprt")
            ) {
                if let Some(name) = path.file_stem().and_then(|value| value.to_str()) {
                    names.insert(name.strip_suffix("_patch").unwrap_or(name).to_string());
                }
            }
        }
    }

    Ok(names
        .into_iter()
        .filter(|name| allowed_paths(name).is_ok())
        .collect())
}

fn backup_of(candidate: &str, original: &str) -> bool {
    let Some(suffix) = candidate.strip_prefix(&format!("{original}.backup")) else {
        return false;
    };
    suffix.is_empty()
        || suffix.strip_prefix('.').is_some_and(|number| {
            !number.is_empty() && number.bytes().all(|digit| digit.is_ascii_digit())
        })
}

impl Preview {
    pub fn scan(game: &Path, appdata: Option<&Path>, name: &str) -> Result<Self, String> {
        let starter = allowed_paths(name)?;
        let mut groups = [BTreeSet::new(), BTreeSet::new(), BTreeSet::new()];
        groups[0].extend(starter.iter().cloned());

        // Everything inside this exact mod folder belongs in the preview. Nothing
        // is deleted recursively: users choose individual files from this list.
        walk(game, &format!("mods/{name}"), &mut groups[1])?;
        groups[1].retain(|candidate| {
            !starter
                .iter()
                .any(|path| path.eq_ignore_ascii_case(candidate))
        });
        for path in &starter {
            let parent = Path::new(path)
                .parent()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            for (candidate, directory) in children(game, &parent)? {
                if !directory && backup_of(&candidate, path) {
                    groups[1].insert(candidate);
                }
            }
        }

        // Shared folders use exact map names, never a broad name-prefix match.
        for suffix in ["", "_patch"] {
            let stem = format!("{name}{suffix}");
            for extension in ["d3dbsp", "d3dprt", "radtrans", "grid_auto"] {
                groups[1].insert(format!("raw/maps/{stem}.{extension}"));
            }
            for extension in ["bak", "autosave", "map.bak"] {
                groups[1].insert(format!("map_source/{stem}.{extension}"));
            }
            for (language, directory) in children(game, "zone_source")? {
                if directory {
                    groups[1].insert(format!("{language}/assetlist/{stem}.csv"));
                    groups[1].insert(format!("{language}/assetinfo/{stem}.csv"));
                    groups[1].insert(format!("{language}/assetinfo/{stem}_xmodel.csv"));
                }
            }
            for (language, directory) in children(game, "zone")? {
                if directory {
                    groups[1].insert(format!("{language}/{stem}.ff"));
                }
            }
        }

        if let Some(root) = appdata {
            walk(root, name, &mut groups[2])?;
        }

        let mut items = Vec::new();
        for (section, paths) in groups.into_iter().enumerate() {
            if paths.is_empty() {
                continue;
            }
            let root = if section == 2 {
                appdata.ok_or("AppData is unavailable.")?
            } else {
                game
            };
            for relative in paths {
                let path = checked_path(root, &relative)?;
                if let Some(hash) = fingerprint(&path)? {
                    items.push(Item {
                        id: items.len(),
                        section,
                        path: relative,
                        bytes: path.metadata().map_err(|error| error.to_string())?.len(),
                        root: root.to_path_buf(),
                        hash,
                    });
                }
            }
        }

        Ok(Self {
            name: name.into(),
            appdata: appdata.map(|path| path.to_string_lossy().into()),
            items,
        })
    }

    pub fn apply(self, selected: Vec<usize>) -> Result<usize, String> {
        let ids: BTreeSet<_> = selected.iter().copied().collect();
        if ids.is_empty()
            || ids.len() != selected.len()
            || ids.iter().any(|id| *id >= self.items.len())
        {
            return Err("Select files from a fresh scan.".into());
        }

        // Check the entire selection before the first removal. A stale preview
        // must not authorize deletion of files edited or replaced since scanning.
        for id in &ids {
            let item = &self.items[*id];
            check_root(&item.root)?;
            let path = checked_path(&item.root, &item.path)?;
            if fingerprint(&path)?.as_ref() != Some(&item.hash) {
                return Err(format!(
                    "{} changed. Scan again before deleting.",
                    item.path
                ));
            }
        }

        let mut deleted = 0;
        for id in ids {
            let item = &self.items[id];
            let remove = || -> Result<(), String> {
                check_root(&item.root)?;
                let path = checked_path(&item.root, &item.path)?;
                if fingerprint(&path)?.as_ref() != Some(&item.hash) {
                    return Err(format!("{} changed during deletion.", item.path));
                }
                fs::remove_file(&path).map_err(|error| error.to_string())?;
                Ok(())
            };
            if let Err(error) = remove() {
                return Err(format!("Stopped after deleting {deleted} files. {error} Scan again to see what remains."));
            }
            deleted += 1;

            // Remove only empty directories, keeping shared top-level folders.
            let mut relative = Path::new(&item.path).parent();
            while let Some(directory) = relative {
                if directory.as_os_str().is_empty()
                    || (item.section != 2 && directory.components().count() <= 1)
                {
                    break;
                }
                let text = directory.to_string_lossy().replace('\\', "/");
                let Ok(path) = checked_path(&item.root, &text) else {
                    break;
                };
                if fs::remove_dir(path).is_err() {
                    break;
                }
                relative = directory.parent();
            }
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, bytes: &[u8]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temporary = tempfile::tempdir().unwrap();
        let game = temporary.path().join("game");
        let appdata = temporary.path().join("appdata/mods");
        fs::create_dir_all(&game).unwrap();
        fs::create_dir_all(&appdata).unwrap();
        for path in allowed_paths("test_map").unwrap() {
            write(&game, &path, b"starter");
        }
        write(&game, "mods/test_map/mod.ff", b"built");
        write(&game, "mods/test_map/custom/readme.txt", b"custom");
        write(&game, "raw/maps/test_map.d3dbsp", b"compiled");
        write(&game, "raw/maps/test_map_extra.d3dbsp", b"neighbour");
        write(&game, "map_source/test_map.map.backup.1", b"backup");
        write(
            &game,
            "zone_source/english/assetinfo/test_map_patch_xmodel.csv",
            b"report",
        );
        write(&appdata, "test_map/mod.ff", b"playable");
        write(&appdata, "test_map_extra/mod.ff", b"neighbour");
        (temporary, game, appdata)
    }

    #[test]
    fn sections_are_disjoint_and_starter_deletion_preserves_other_files() {
        let (_temporary, game, appdata) = fixture();
        let preview = Preview::scan(&game, Some(&appdata), "test_map").unwrap();
        let counts: Vec<_> = (0..3)
            .map(|section| {
                preview
                    .items
                    .iter()
                    .filter(|item| item.section == section)
                    .count()
            })
            .collect();
        assert_eq!(counts, [18, 5, 1]);
        let selected = preview
            .items
            .iter()
            .filter(|item| item.section == 0)
            .map(|item| item.id)
            .collect();
        assert_eq!(preview.apply(selected).unwrap(), 18);
        assert!(game.join("mods/test_map/mod.ff").exists());
        assert!(game.join("map_source/test_map.map.backup.1").exists());
        assert!(appdata.join("test_map/mod.ff").exists());
    }

    #[test]
    fn complete_cleanup_preserves_neighbours_and_shared_folders() {
        let (_temporary, game, appdata) = fixture();
        let preview = Preview::scan(&game, Some(&appdata), "test_map").unwrap();
        let selected = preview.items.iter().map(|item| item.id).collect();
        assert_eq!(preview.apply(selected).unwrap(), 24);
        assert!(!game.join("mods/test_map").exists());
        assert!(!appdata.join("test_map").exists());
        assert!(game.join("mods").exists());
        assert!(game.join("map_source").exists());
        assert!(game.join("raw/maps/test_map_extra.d3dbsp").exists());
        assert!(appdata.join("test_map_extra/mod.ff").exists());
    }

    #[test]
    fn changed_preview_and_invalid_selections_delete_nothing() {
        let (_temporary, game, appdata) = fixture();
        let preview = Preview::scan(&game, Some(&appdata), "test_map").unwrap();
        assert!(preview.clone().apply(vec![999]).is_err());
        assert!(preview.clone().apply(vec![0, 0]).is_err());
        assert!(preview.clone().apply(vec![]).is_err());
        let selected = preview.items.iter().map(|item| item.id).collect();
        write(&game, "mods/test_map/mod.ff", b"changed");
        assert!(preview.apply(selected).is_err());
        assert!(game.join("map_source/test_map.map").exists());
        assert!(appdata.join("test_map/mod.ff").exists());
        assert!(Preview::scan(&game, Some(&appdata), "../test_map").is_err());
    }

    #[test]
    fn scan_is_read_only_and_appdata_can_be_absent() {
        let temporary = tempfile::tempdir().unwrap();
        let game = temporary.path().join("game");
        fs::create_dir(&game).unwrap();
        assert!(Preview::scan(&game, None, "test_map")
            .unwrap()
            .items
            .is_empty());
        assert_eq!(fs::read_dir(&game).unwrap().count(), 0);
        write(&game, "raw/maps/test_map.d3dbsp", b"leftover");
        assert!(maps(&game, None).unwrap().contains(&"test_map".into()));
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_rejected_without_touching_their_targets() {
        let (_temporary, game, appdata) = fixture();
        let target = game.join("outside");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("keep.txt"), b"keep").unwrap();
        let link = game.join("mods/test_map/linked");
        let result = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link.to_string_lossy().replace('/', "\\"))
            .arg(target.to_string_lossy().replace('/', "\\"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{} {}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Preview::scan(&game, Some(&appdata), "test_map");
        fs::remove_dir(&link).unwrap();
        assert!(result.is_err());
        assert_eq!(fs::read(target.join("keep.txt")).unwrap(), b"keep");
    }
}
