// TBG Pricing — portable desktop shell: local data persistence + self-updating exe.
//
// Distribution model (v0.2.0): a single PORTABLE .exe living in a folder the boss
// controls (e.g. Desktop\pricing\), placed there by installer.bat. There is no
// NSIS installer and no Program Files / registry footprint.
//
// Persistence: the built/enriched catalogue is saved next to the exe under
// .\data\catalogue.json (commands `save_catalogue` / `load_catalogue`). On launch
// the front-end reloads it, so the app opens straight into the comparison instead
// of the Upload screen. Data never leaves the boss's machine.
//
// Auto-update: on launch we ask the GitHub-hosted `latest.json` whether a newer
// release exists (via tauri-plugin-updater, which downloads AND verifies the
// minisign signature for us). Because the app is portable — NOT installed via
// NSIS — we do NOT call the plugin's installer. Instead we take the verified new
// exe bytes and swap them in while the exe is idle: Windows lets you rename a
// running .exe but not overwrite it, so we move the current exe to <exe>.old and
// drop the new bytes into its place. The swap happens on "Reiniciar y actualizar"
// (`install_now`) or as the window closes (`ExitRequested`); the leftover
// <exe>.old is deleted on the next launch.
//
// Everything degrades gracefully: no network / no release / bad signature / a
// read-only folder just leaves the app running as a normal offline viewer.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_updater::{Update, UpdaterExt};

/// A downloaded-but-not-yet-applied update: the plugin's handle (kept for its
/// version string) plus the verified new-exe bytes, held until the user installs
/// it or closes the window.
#[derive(Default)]
struct Pending(Mutex<Option<(Update, Vec<u8>)>>);

// ---- portable data persistence (next to the running .exe) ------------------

/// `<dir of the running exe>\data`, created on demand.
fn data_dir() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "exe has no parent directory".to_string())?
        .join("data");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn catalogue_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("catalogue.json"))
}

/// Persist the built/enriched catalogue (a JSON array) beside the exe.
#[tauri::command]
fn save_catalogue(json: String) -> Result<(), String> {
    fs::write(catalogue_path()?, json).map_err(|e| e.to_string())
}

/// Load the persisted catalogue, or `None` if nothing has been saved yet.
#[tauri::command]
fn load_catalogue() -> Option<String> {
    fs::read_to_string(catalogue_path().ok()?).ok()
}

// ---- portable update swap --------------------------------------------------

/// A sibling of the running exe, e.g. `TBG.Pricing.exe` + ".old" -> `TBG.Pricing.exe.old`.
fn exe_sibling(suffix: &str) -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let name = exe
        .file_name()
        .ok_or_else(|| "exe has no file name".to_string())?
        .to_string_lossy()
        .into_owned();
    let dir = exe
        .parent()
        .ok_or_else(|| "exe has no parent directory".to_string())?;
    Ok(dir.join(format!("{name}{suffix}")))
}

/// Swap the freshly downloaded bytes in for the running exe. Windows allows
/// renaming a running executable (but not overwriting it), so we move the current
/// exe aside to <exe>.old and put the new bytes in its place; the stale .old is
/// cleaned up on the next boot. Rolls back on failure so a runnable exe remains.
fn apply_portable_update(bytes: &[u8]) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let new_path = exe_sibling(".new")?;
    let old_path = exe_sibling(".old")?;

    fs::write(&new_path, bytes).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&old_path);
    fs::rename(&exe, &old_path).map_err(|e| e.to_string())?;
    if let Err(e) = fs::rename(&new_path, &exe) {
        // Roll back; if the rollback rename ALSO fails, the canonical path is
        // still free, so drop the new bytes directly there — a runnable exe must
        // always remain (best-effort: no-op if that path is itself locked).
        if fs::rename(&old_path, &exe).is_err() {
            let _ = fs::write(&exe, bytes);
        }
        return Err(e.to_string());
    }
    Ok(())
}

/// Remove leftover `<exe>.old` / `<exe>.new` staging files from a previous
/// update, if any. Safe at launch: swaps only run at ExitRequested or inside
/// install_now (which completes the swap before spawning), so a fresh process
/// running this in setup never races an in-progress swap.
fn cleanup_previous_update() {
    for suffix in [".old", ".new"] {
        if let Ok(p) = exe_sibling(suffix) {
            let _ = fs::remove_file(p);
        }
    }
}

// ---- commands --------------------------------------------------------------

#[tauri::command]
fn current_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Manual "check for updates". Checks, and if there is one, downloads + verifies
/// it and stashes the bytes so `install_now` / the on-close handler can swap it in.
/// Returns the new version string, or `None` if already up to date.
#[tauri::command]
async fn check_now(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await.map_err(|e| e.to_string())? {
        Some(update) => {
            let version = update.version.clone();
            let bytes = update
                .download(|_chunk, _total| {}, || {})
                .await
                .map_err(|e| e.to_string())?;
            *app.state::<Pending>().0.lock().unwrap() = Some((update, bytes));
            let _ = app.emit("update://ready", &version);
            Ok(Some(version))
        }
        None => Ok(None),
    }
}

/// Swap the already-downloaded update in and relaunch into it.
#[tauri::command]
async fn install_now(app: tauri::AppHandle) -> Result<(), String> {
    let pending = app.state::<Pending>().0.lock().unwrap().take();
    let (_update, bytes) = pending.ok_or_else(|| "no update downloaded".to_string())?;
    // Capture the exe path BEFORE the swap: apply_portable_update renames the
    // running exe to <exe>.old, after which current_exe() (and thus app.restart())
    // would point at the OLD file. So we relaunch the original path — which now
    // holds the NEW bytes — explicitly, then quit.
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    apply_portable_update(&bytes)?;
    std::process::Command::new(&exe)
        .spawn()
        .map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(Pending::default())
        .invoke_handler(tauri::generate_handler![
            current_version,
            check_now,
            install_now,
            save_catalogue,
            load_catalogue
        ])
        .setup(|app| {
            cleanup_previous_update();
            // Fire-and-forget update check on launch.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let updater = match handle.updater() {
                    Ok(u) => u,
                    Err(_) => return,
                };
                match updater.check().await {
                    Ok(Some(update)) => {
                        let version = update.version.clone();
                        let _ = handle.emit("update://available", &version);
                        match update.download(|_c, _t| {}, || {}).await {
                            Ok(bytes) => {
                                *handle.state::<Pending>().0.lock().unwrap() =
                                    Some((update, bytes));
                                let _ = handle.emit("update://ready", &version);
                            }
                            Err(e) => {
                                let _ = handle.emit("update://error", e.to_string());
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => {
                        let _ = handle.emit("update://error", e.to_string());
                    }
                }
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building TBG Pricing")
        .run(|handle, event| {
            // Swap in a downloaded update as the app exits, while the exe is idle —
            // Windows can only replace the .exe while it is not running.
            if let RunEvent::ExitRequested { .. } = event {
                let pending = handle.state::<Pending>().0.lock().unwrap().take();
                if let Some((_update, bytes)) = pending {
                    let _ = apply_portable_update(&bytes);
                }
            }
        });
}
