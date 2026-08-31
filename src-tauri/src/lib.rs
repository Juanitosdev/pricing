// TBG Pricing — desktop shell with GitHub-backed auto-update.
//
// Flow (matches the product requirement "descarga la nueva versión y, cuando el
// panel no está abierto, reemplaza el exe viejo por el nuevo"):
//   1. On launch a background task asks the updater endpoint whether a newer
//      release exists.  If so it emits `update://available` and downloads the
//      new bundle straight away, stashing the bytes in app state, then emits
//      `update://ready`.
//   2. The front-end shows an unobtrusive bar.  The user can hit "Reiniciar y
//      actualizar" (`install_now` → installs + relaunches immediately), OR just
//      keep working.
//   3. When the window is closed with a downloaded update pending, we install it
//      on the way out (`ExitRequested`) — Windows can only swap the .exe while
//      it is not running, so the replacement happens exactly "cuando el panel no
//      está abierto".
//
// Everything degrades gracefully: no network / no release / bad signature just
// leaves the app running as a normal offline viewer.

use std::sync::Mutex;
use tauri::{Emitter, Manager, RunEvent};
use tauri_plugin_updater::{Update, UpdaterExt};

/// A downloaded-but-not-yet-installed update, kept until the user installs it or
/// closes the window.
#[derive(Default)]
struct Pending(Mutex<Option<(Update, Vec<u8>)>>);

#[tauri::command]
fn current_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

/// Manual "check for updates". Checks, and if there is one, downloads it and
/// stashes it so `install_now` / the on-close handler can apply it.
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

/// Install the already-downloaded update and relaunch into it.
#[tauri::command]
async fn install_now(app: tauri::AppHandle) -> Result<(), String> {
    let pending = app.state::<Pending>().0.lock().unwrap().take();
    let (update, bytes) = pending.ok_or_else(|| "no update downloaded".to_string())?;
    update.install(bytes).map_err(|e| e.to_string())?;
    // restart() diverges (-> !); it coerces to the Result return type as the tail
    // expression, so no trailing Ok(()) is needed (and would be unreachable).
    app.restart()
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
            install_now
        ])
        .setup(|app| {
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
            // Apply a pending update as the app exits, so the .exe is swapped
            // while it is not running.
            if let RunEvent::ExitRequested { .. } = event {
                let pending = handle.state::<Pending>().0.lock().unwrap().take();
                if let Some((update, bytes)) = pending {
                    let _ = update.install(bytes);
                }
            }
        });
}
