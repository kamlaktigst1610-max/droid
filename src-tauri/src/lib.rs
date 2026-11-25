mod adb;
mod avd;
mod emulator;
mod proc;
mod profiles;
mod sdk;
mod settings;

use proc::Logger;
use serde::Serialize;
use settings::Settings;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};

const MAX_LOG_LINES: usize = 2000;

#[derive(Serialize, Clone, Debug)]
pub struct LogLine {
    ts: u64,
    level: String,
    msg: String,
}

struct Inner {
    registry: emulator::Registry,
    logs: Mutex<VecDeque<LogLine>>,
    settings_path: PathBuf,
    settings: Mutex<Settings>,
}

pub struct AppState(Arc<Inner>);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn push_log(app: &AppHandle, inner: &Inner, level: &str, msg: String) {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let line = LogLine {
        ts,
        level: level.to_string(),
        msg,
    };
    {
        let mut logs = lock(&inner.logs);
        logs.push_back(line.clone());
        while logs.len() > MAX_LOG_LINES {
            logs.pop_front();
        }
    }
    let _ = app.emit("droidlite-log", line);
}

fn make_logger(app: &AppHandle, inner: &Arc<Inner>) -> Logger {
    let app = app.clone();
    let inner = inner.clone();
    Arc::new(move |level: &str, msg: String| push_log(&app, &inner, level, msg))
}

fn current_sdk(inner: &Inner) -> sdk::SdkInfo {
    let path = lock(&inner.settings).sdk_path.clone();
    sdk::detect(path.as_deref())
}

/// Runs blocking work (process execution) off the UI thread.
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn detect_sdk(state: State<'_, AppState>) -> Result<sdk::SdkInfo, String> {
    let inner = state.0.clone();
    blocking(move || Ok(current_sdk(&inner))).await
}

#[tauri::command]
async fn check_acceleration(state: State<'_, AppState>) -> Result<emulator::AccelInfo, String> {
    let inner = state.0.clone();
    blocking(move || Ok(emulator::accel_check(&current_sdk(&inner)))).await
}

#[tauri::command]
fn get_profiles() -> Vec<profiles::DeviceProfile> {
    profiles::all()
}

#[tauri::command]
async fn list_devices() -> Result<Vec<avd::AvdInfo>, String> {
    blocking(|| Ok(avd::list_avds())).await
}

#[tauri::command]
async fn device_statuses(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<emulator::DeviceStatus>, String> {
    let inner = state.0.clone();
    let log = make_logger(&app, &inner);
    blocking(move || {
        let sdk = current_sdk(&inner);
        let names: Vec<String> = avd::list_avds().into_iter().map(|a| a.name).collect();
        Ok(emulator::statuses(&sdk, &names, &inner.registry, &log))
    })
    .await
}

#[tauri::command]
async fn create_device(
    app: AppHandle,
    state: State<'_, AppState>,
    req: avd::CreateRequest,
) -> Result<String, String> {
    let inner = state.0.clone();
    let log = make_logger(&app, &inner);
    blocking(move || {
        let result = avd::create(&current_sdk(&inner), &req, &log);
        if let Err(e) = &result {
            log("error", format!("Create device failed: {e}"));
        }
        result
    })
    .await
}

#[tauri::command]
async fn start_device(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<String, String> {
    let inner = state.0.clone();
    let log = make_logger(&app, &inner);
    blocking(move || {
        let settings = lock(&inner.settings).clone();
        let result = emulator::start(&current_sdk(&inner), &settings, &name, &inner.registry, &log);
        if let Err(e) = &result {
            log("error", format!("Start failed: {e}"));
        }
        result
    })
    .await
}

#[tauri::command]
async fn stop_device(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<(), String> {
    let inner = state.0.clone();
    let log = make_logger(&app, &inner);
    blocking(move || {
        let result = emulator::stop(&current_sdk(&inner), &name, &inner.registry, &log);
        if let Err(e) = &result {
            log("error", format!("Stop failed: {e}"));
        }
        result
    })
    .await
}

#[tauri::command]
async fn install_apk(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    path: String,
) -> Result<String, String> {
    let inner = state.0.clone();
    let log = make_logger(&app, &inner);
    blocking(move || {
        let sdk = current_sdk(&inner);
        let adb = adb::from_sdk(&sdk)?;
        let serial = emulator::serial_for(&adb, &name, &inner.registry)
            .ok_or_else(|| format!("{name} is not running. Start it first."))?;
        if adb.getprop(&serial, "sys.boot_completed").as_deref() != Some("1") {
            return Err(format!("{name} is still booting. Try again when it shows Ready."));
        }
        log("info", format!("Installing {path} on {name}"));
        let result = adb.install(&serial, &path);
        match &result {
            Ok(_) => log("info", format!("Installed {path} on {name}")),
            Err(e) => log("error", format!("Install failed: {e}")),
        }
        result
    })
    .await
}

#[tauri::command]
fn get_settings(state: State<'_, AppState>) -> Settings {
    lock(&state.0.settings).clone()
}

#[tauri::command]
fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    let mut s = settings;
    if !settings::GPU_MODES.contains(&s.gpu_mode.as_str()) {
        return Err("Unknown graphics mode.".to_string());
    }
    s.sdk_path = s
        .sdk_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());
    settings::save(&state.0.settings_path, &s)?;
    *lock(&state.0.settings) = s.clone();
    Ok(s)
}

#[tauri::command]
fn get_logs(state: State<'_, AppState>) -> Vec<LogLine> {
    lock(&state.0.logs).iter().cloned().collect()
}

#[tauri::command]
fn clear_logs(state: State<'_, AppState>) {
    lock(&state.0.logs).clear();
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
            let _ = std::fs::create_dir_all(&dir);
            let settings_path = dir.join("settings.json");
            let loaded = settings::load(&settings_path);
            app.manage(AppState(Arc::new(Inner {
                registry: emulator::new_registry(),
                logs: Mutex::new(VecDeque::new()),
                settings_path,
                settings: Mutex::new(loaded),
            })));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            detect_sdk,
            check_acceleration,
            get_profiles,
            list_devices,
            device_statuses,
            create_device,
            start_device,
            stop_device,
            install_apk,
            get_settings,
            set_settings,
            get_logs,
            clear_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running DroidLite");
}
