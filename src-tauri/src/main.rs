#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod delete;
mod files;

use files::{Entry, Plan, Report};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};

struct AppState {
    root: PathBuf,
    plan: Mutex<Option<Plan>>,
    deletion: Mutex<Option<delete::Preview>>,
}

#[derive(Serialize)]
struct Context {
    root: String,
    map_source: String,
    development: bool,
    error: Option<String>,
}

// Check the game executable, not the folder name or the user's choice of patches.
fn root_error(root: &std::path::Path) -> Option<String> {
    if !root.join("CoDWaW.exe").is_file() {
        return Some("Put this executable in your World at War folder, beside CoDWaW.exe, then open it again.".into());
    }
    None
}

fn production_root_error(root: &std::path::Path) -> Option<String> {
    if cfg!(debug_assertions) {
        None
    } else {
        root_error(root)
    }
}

#[tauri::command]
fn quit(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn context(state: tauri::State<AppState>) -> Context {
    Context {
        root: state.root.to_string_lossy().into(),
        map_source: state.root.join("map_source").to_string_lossy().into(),
        development: cfg!(debug_assertions),
        error: production_root_error(&state.root),
    }
}

#[tauri::command]
fn read_custom(path: String) -> Result<Vec<u8>, String> {
    let path = PathBuf::from(path);
    if path.extension().and_then(|s| s.to_str()) != Some("map") {
        return Err("Choose a file with a .map extension.".into());
    }

    let metadata = path.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("Choose a .map file.".into());
    }
    std::fs::read(path).map_err(|e| e.to_string())
}

#[tauri::command]
fn prepare(
    name: String,
    entries: Vec<Entry>,
    state: tauri::State<AppState>,
) -> Result<Vec<String>, String> {
    // Discard any earlier review before preparing a new set of files.
    let mut stored = state.plan.lock().map_err(|e| e.to_string())?;
    *stored = None;
    if let Some(error) = production_root_error(&state.root) {
        return Err(error);
    }

    let plan = Plan::prepare(&state.root, &name, entries)?;
    let conflicts = plan.conflicts();
    *stored = Some(plan);
    Ok(conflicts)
}

#[tauri::command]
fn apply(
    choices: HashMap<String, String>,
    state: tauri::State<AppState>,
) -> Result<Report, String> {
    // Consume the plan so the same overwrite decision cannot be applied twice.
    state
        .plan
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("Generate again to prepare the files.")?
        .apply(choices)
}

#[tauri::command]
fn delete_maps(state: tauri::State<AppState>) -> Result<Vec<String>, String> {
    if let Some(error) = production_root_error(&state.root) {
        return Err(error);
    }
    delete::maps(&state.root, delete::appdata_root(&state.root).as_deref())
}

#[tauri::command]
fn delete_scan(name: String, state: tauri::State<AppState>) -> Result<delete::Preview, String> {
    let mut stored = state.deletion.lock().map_err(|error| error.to_string())?;
    *stored = None;
    if let Some(error) = production_root_error(&state.root) {
        return Err(error);
    }
    let preview = delete::Preview::scan(
        &state.root,
        delete::appdata_root(&state.root).as_deref(),
        &name,
    )?;
    *stored = Some(preview.clone());
    Ok(preview)
}

#[tauri::command]
fn delete_apply(selected: Vec<usize>, state: tauri::State<AppState>) -> Result<usize, String> {
    if let Some(error) = production_root_error(&state.root) {
        return Err(error);
    }
    state
        .deletion
        .lock()
        .map_err(|error| error.to_string())?
        .take()
        .ok_or("Scan again before deleting.")?
        .apply(selected)
}

fn main() {
    // Development uses a test folder. Releases use the EXE's own folder each run.
    let root = if cfg!(debug_assertions) {
        std::env::var_os("WAW_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.local/test-root")
            })
    } else {
        std::env::current_exe()
            .expect("Cannot locate the executable")
            .parent()
            .expect("Cannot locate the app folder")
            .to_path_buf()
    };

    if cfg!(debug_assertions) {
        std::fs::create_dir_all(root.join("map_source"))
            .expect("Cannot create the development output folder");
    }

    // This app has no browser state to retain. Give each running copy its own
    // profile so a WebView still closing elsewhere cannot lock the next launch.
    let webview_profile = tempfile::Builder::new()
        .prefix("waw-script-placer-")
        .tempdir()
        .expect("Cannot create the temporary browser profile");
    let profile_path = webview_profile.path().to_path_buf();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            tauri::WebviewWindowBuilder::from_config(app, &app.config().app.windows[0])?
                .data_directory(profile_path)
                .build()?;
            Ok(())
        })
        .manage(AppState {
            root,
            plan: Mutex::new(None),
            deletion: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            context,
            quit,
            read_custom,
            prepare,
            apply,
            delete_maps,
            delete_scan,
            delete_apply
        ])
        .run(tauri::generate_context!())
        .expect("Cannot start WaW Script Placer");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_validation_uses_contents_not_folder_name() {
        let root = tempfile::tempdir().unwrap();
        assert!(root_error(root.path()).is_some());
        std::fs::write(root.path().join("CoDWaW.exe"), b"test marker").unwrap();
        assert!(root_error(root.path()).is_none());
        std::fs::remove_file(root.path().join("CoDWaW.exe")).unwrap();
        assert!(root_error(root.path()).is_some());
    }
}
