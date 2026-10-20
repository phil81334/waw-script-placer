use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::{Component, Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Deserialize)]
pub struct Entry {
    pub path: String,
    pub bytes: Vec<u8>,
}

// Keep hashes of the reviewed files to detect changes made during the dialog.
pub struct Plan {
    root: PathBuf,
    entries: Vec<Entry>,
    before: Vec<Option<String>>,
}

#[derive(Serialize)]
pub struct Report {
    pub written: usize,
    pub skipped: usize,
    pub backups: Vec<String>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// Windows reparse points include junctions as well as symbolic links.
fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

// Reject paths that could escape the selected root, including linked directories.
pub(crate) fn checked_path(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let rel = Path::new(relative);
    if relative.contains('\\')
        || relative.contains(':')
        || rel.components().any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("Invalid output path.".into());
    }

    let mut path = root.to_path_buf();
    for component in rel.components() {
        path.push(component);
        match fs::symlink_metadata(&path) {
            Ok(meta) if is_link(&meta) => {
                return Err(format!("Linked output paths are not supported: {relative}"))
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => {
                return Err(err(e));
            }
        }
    }
    Ok(path)
}

pub(crate) fn fingerprint(path: &Path) -> Result<Option<String>, String> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if is_link(&meta) || !meta.is_file() {
                return Err(format!("Not a regular file: {}", path.display()));
            }
            Ok(Some(format!(
                "{:x}",
                Sha256::digest(fs::read(path).map_err(err)?)
            )))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(err(e)),
    }
}

// Accept only the 18 destinations used by General generation.
pub fn allowed_paths(name: &str) -> Result<HashSet<String>, String> {
    let first = name.bytes().next().ok_or("Enter a map name.")?;
    let reserved = name.to_ascii_lowercase();
    if !(first.is_ascii_alphabetic() || first == b'_')
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || ["con", "prn", "aux", "nul"].contains(&reserved.as_str())
        || ((reserved.starts_with("com") || reserved.starts_with("lpt"))
            && reserved.len() == 4
            && reserved.as_bytes()[3].is_ascii_digit())
    {
        return Err("Invalid map name.".into());
    }

    let mut paths = HashSet::new();
    for suffix in ["", "_patch"] {
        paths.insert(format!("map_source/{name}{suffix}.map"));
        paths.insert(format!("zone_source/{name}{suffix}.csv"));
        paths.insert(format!("bin/Launcher/map_settings/{name}{suffix}.cfg"));
    }

    for file in [
        format!("{name}.files"),
        "DLC3_Weapons.iwd".into(),
        "mod.arena".into(),
        "mod.csv".into(),
        format!("maps/{name}.gsc"),
        "maps/_loadout.gsc".into(),
        "maps/mapsTable.csv".into(),
        format!("maps/createart/{name}_art.gsc"),
        format!("maps/createfx/{name}_fx.gsc"),
        format!("clientscripts/{name}.csc"),
        format!("clientscripts/{name}_amb.csc"),
        format!("clientscripts/createfx/{name}_fx.csc"),
    ] {
        paths.insert(format!("mods/{name}/{file}"));
    }
    Ok(paths)
}

impl Plan {
    // Preparation reads destinations but does not create or overwrite anything.
    pub fn prepare(root: &Path, name: &str, entries: Vec<Entry>) -> Result<Self, String> {
        let allowed = allowed_paths(name)?;
        let actual: HashSet<_> = entries.iter().map(|e| e.path.clone()).collect();
        if entries.len() != 18 || actual != allowed {
            return Err("Expected exactly the 18 starter files.".into());
        }

        let root = root.canonicalize().map_err(err)?;
        let before = entries
            .iter()
            .map(|entry| fingerprint(&checked_path(&root, &entry.path)?))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            root,
            entries,
            before,
        })
    }

    pub fn conflicts(&self) -> Vec<String> {
        self.entries
            .iter()
            .zip(&self.before)
            .filter(|(_, hash)| hash.is_some())
            .map(|(e, _)| e.path.clone())
            .collect()
    }

    pub fn apply(self, choices: HashMap<String, String>) -> Result<Report, String> {
        // Validate the whole decision and recheck every destination before writing.
        for (entry, before) in self.entries.iter().zip(&self.before) {
            let path = checked_path(&self.root, &entry.path)?;
            if fingerprint(&path)? != *before {
                return Err(format!(
                    "{} changed. Generate again to review the new files.",
                    entry.path
                ));
            }

            if before.is_some()
                && !matches!(
                    choices.get(&entry.path).map(String::as_str),
                    Some("skip" | "replace" | "backup")
                )
            {
                return Err("Choose what to do with each existing file.".into());
            }
        }

        // Finish writing temporary files before replacing any destination.
        // Skipped files never enter this list.
        let mut staged = Vec::new();
        let mut report = Report {
            written: 0,
            skipped: 0,
            backups: Vec::new(),
        };

        for (entry, before) in self.entries.iter().zip(&self.before) {
            if before.is_some() && choices.get(&entry.path).map(String::as_str) == Some("skip") {
                report.skipped += 1;
                continue;
            }

            let path = checked_path(&self.root, &entry.path)?;
            fs::create_dir_all(path.parent().ok_or("Invalid output directory.")?).map_err(err)?;
            checked_path(&self.root, &entry.path)?;
            let mut temp = NamedTempFile::new_in(path.parent().unwrap()).map_err(err)?;
            temp.write_all(&entry.bytes).map_err(err)?;
            temp.as_file().sync_all().map_err(err)?;
            staged.push((entry, before, path, temp));
        }

        for (entry, before, path, temp) in staged {
            let commit = || -> Result<Option<String>, String> {
                checked_path(&self.root, &entry.path)?;
                if fingerprint(&path)? != *before {
                    return Err(format!("{} changed during generation.", entry.path));
                }

                let mut backup = None;
                if before.is_some()
                    && choices.get(&entry.path).map(String::as_str) == Some("backup")
                {
                    let bytes = fs::read(&path).map_err(err)?;

                    // create_new ensures an existing backup is never overwritten.
                    for number in 0..10000 {
                        let relative = if number == 0 {
                            format!("{}.backup", entry.path)
                        } else {
                            format!("{}.backup.{number}", entry.path)
                        };

                        let target = checked_path(&self.root, &relative)?;
                        match fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&target)
                        {
                            Ok(mut file) => {
                                file.write_all(&bytes).map_err(err)?;
                                file.sync_all().map_err(err)?;
                                backup = Some(relative);
                                break;
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                                continue;
                            }
                            Err(e) => {
                                return Err(err(e));
                            }
                        }
                    }

                    if backup.is_none() {
                        return Err("Too many backups for this file.".into());
                    }
                }

                if before.is_none() {
                    temp.persist_noclobber(&path).map_err(err)?;
                } else {
                    temp.persist(&path).map_err(err)?;
                }
                Ok(backup)
            };
            match commit() {
                Ok(backup) => {
                    report.written += 1;

                    if let Some(path) = backup {
                        report.backups.push(path);
                    }
                }
                Err(e) => {
                    return Err(format!("Stopped after writing {} files. {e} Files already written and any backups remain in the selected root.", report.written));
                }
            }
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entries() -> Vec<Entry> {
        allowed_paths("test_map")
            .unwrap()
            .into_iter()
            .map(|path| Entry {
                path,
                bytes: b"new".to_vec(),
            })
            .collect()
    }

    fn decisions(plan: &Plan, action: &str) -> HashMap<String, String> {
        plan.conflicts()
            .into_iter()
            .map(|p| (p, action.into()))
            .collect()
    }

    #[test]
    fn create_skip_replace_and_back_up() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            Plan::prepare(dir.path(), "test_map", entries())
                .unwrap()
                .apply(HashMap::new())
                .unwrap()
                .written,
            18
        );
        let file = dir.path().join("map_source/test_map.map");
        fs::write(&file, "original").unwrap();
        let plan = Plan::prepare(dir.path(), "test_map", entries()).unwrap();
        let actions = decisions(&plan, "skip");
        assert_eq!(plan.apply(actions).unwrap().skipped, 18);
        assert_eq!(fs::read(&file).unwrap(), b"original");
        for old in ["original", "second"] {
            fs::write(&file, old).unwrap();
            let plan = Plan::prepare(dir.path(), "test_map", entries()).unwrap();
            let actions = decisions(&plan, "backup");
            assert_eq!(plan.apply(actions).unwrap().backups.len(), 18);
        }
        assert_eq!(
            fs::read(file.with_extension("map.backup")).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(file.with_extension("map.backup.1")).unwrap(),
            b"second"
        );
        fs::write(&file, "replace me").unwrap();
        let plan = Plan::prepare(dir.path(), "test_map", entries()).unwrap();
        let actions = decisions(&plan, "replace");
        assert!(plan.apply(actions).unwrap().backups.is_empty());
        assert_eq!(fs::read(&file).unwrap(), b"new");
    }

    #[test]
    fn missing_choices_write_nothing_and_skip_preserves_existing_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let map = dir.path().join("map_source/test_map.map");
        fs::create_dir(map.parent().unwrap()).unwrap();
        fs::write(&map, b"UGX_SCRIPTPLACER_MAPNAME\nuser changes").unwrap();
        let plan = Plan::prepare(dir.path(), "test_map", entries()).unwrap();
        assert!(plan.apply(HashMap::new()).is_err());
        assert!(!dir.path().join("mods").exists());
        let plan = Plan::prepare(dir.path(), "test_map", entries()).unwrap();
        let actions = decisions(&plan, "skip");
        let report = plan.apply(actions).unwrap();
        assert_eq!(report.written, 17);
        assert_eq!(report.skipped, 1);
        assert_eq!(
            fs::read(map).unwrap(),
            b"UGX_SCRIPTPLACER_MAPNAME\nuser changes"
        );
    }

    #[test]
    fn refuses_changed_files_and_unexpected_paths() {
        let dir = tempfile::tempdir().unwrap();
        let plan = Plan::prepare(dir.path(), "test_map", entries()).unwrap();
        fs::create_dir(dir.path().join("map_source")).unwrap();
        fs::write(dir.path().join("map_source/test_map.map"), "appeared").unwrap();
        assert!(plan.apply(HashMap::new()).is_err());
        assert!(!dir.path().join("mods").exists());
        let mut bad = entries();
        bad[0].path = "../outside".into();
        assert!(Plan::prepare(dir.path(), "test_map", bad).is_err());
        assert!(allowed_paths("CON").is_err());
        assert!(allowed_paths("../bad").is_err());
    }
}
