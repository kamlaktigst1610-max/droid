//! Starting, stopping and monitoring emulator processes.

use crate::adb::{self, Adb};
use crate::proc::{self, Logger};
use crate::sdk::SdkInfo;
use crate::settings::Settings;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// An emulator process that DroidLite started.
pub struct Managed {
    child: Child,
    serial: String,
    started: Instant,
    stopping: bool,
    booted: bool,
}

pub type Registry = Mutex<HashMap<String, Managed>>;

pub fn new_registry() -> Registry {
    Mutex::new(HashMap::new())
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Serialize, Clone, Debug)]
pub struct AccelInfo {
    pub available: bool,
    pub detail: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct DeviceStatus {
    pub name: String,
    /// stopped | starting | running | stopping
    pub state: String,
    pub stage: String,
    /// 0–100
    pub progress: u8,
    pub serial: Option<String>,
    /// true when DroidLite started this emulator
    pub managed: bool,
}

fn stopped(name: &str) -> DeviceStatus {
    DeviceStatus {
        name: name.to_string(),
        state: "stopped".to_string(),
        stage: "Stopped".to_string(),
        progress: 0,
        serial: None,
        managed: false,
    }
}

pub fn accel_check(sdk: &SdkInfo) -> AccelInfo {
    let Some(emu) = sdk.emulator.as_ref() else {
        return AccelInfo {
            available: false,
            detail: "Android Emulator is not installed.".to_string(),
        };
    };
    match proc::run(Path::new(emu), &["-accel-check"], None, Duration::from_secs(20)) {
        Ok(out) => {
            let text = out.combined();
            let available =
                out.success() || text.to_lowercase().contains("is installed and usable");
            let detail = text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && *l != "accel" && *l != "0" && *l != "1")
                .last()
                .unwrap_or("No details reported.")
                .to_string();
            AccelInfo { available, detail }
        }
        Err(e) => AccelInfo {
            available: false,
            detail: e,
        },
    }
}

fn port_free(p: u16) -> bool {
    TcpListener::bind(("127.0.0.1", p)).is_ok()
}

fn serial_port(serial: &str) -> Option<u16> {
    serial.strip_prefix("emulator-")?.parse().ok()
}

fn pick_port(reg: &Registry, adb: &Adb) -> Option<u16> {
    let mut used: HashSet<u16> = HashSet::new();
    for m in lock(reg).values() {
        if let Some(p) = serial_port(&m.serial) {
            used.insert(p);
        }
    }
    if let Ok(devs) = adb.devices() {
        for (s, _) in devs {
            if let Some(p) = serial_port(&s) {
                used.insert(p);
            }
        }
    }
    (5554u16..=5584)
        .step_by(2)
        .find(|p| !used.contains(p) && port_free(*p) && port_free(*p + 1))
}

/// Emulators running outside DroidLite: AVD name -> serial.
fn external(adb: &Adb, devs: &[(String, String)], reg: &Registry) -> HashMap<String, String> {
    let managed: HashSet<String> = lock(reg).values().map(|m| m.serial.clone()).collect();
    let mut map = HashMap::new();
    for (serial, _) in devs {
        if serial.starts_with("emulator-") && !managed.contains(serial) {
            if let Some(n) = adb.avd_name(serial) {
                map.insert(n, serial.clone());
            }
        }
    }
    map
}

fn pump<R: Read + Send + 'static>(r: R, log: Logger) {
    thread::spawn(move || {
        let mut rd = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match rd.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    let level = if line.contains("ERROR") || line.contains("FATAL") {
                        "error"
                    } else if line.contains("WARNING") {
                        "warn"
                    } else {
                        "info"
                    };
                    log(level, format!("[emulator] {line}"));
                }
            }
        }
    });
}

/// Launches the real Android Emulator for `name`. Returns its adb serial.
pub fn start(
    sdk: &SdkInfo,
    settings: &Settings,
    name: &str,
    reg: &Registry,
    log: &Logger,
) -> Result<String, String> {
    let emu = sdk
        .emulator
        .clone()
        .ok_or_else(|| "The Android Emulator package isn’t installed.".to_string())?;
    let adb = adb::from_sdk(sdk)?;

    if lock(reg).contains_key(name) {
        return Err(format!("{name} is already running."));
    }
    let devs = adb.devices().unwrap_or_default();
    if external(&adb, &devs, reg).contains_key(name) {
        return Err(format!("{name} is already running outside DroidLite."));
    }
    let port =
        pick_port(reg, &adb).ok_or_else(|| "No free emulator port between 5554 and 5585.".to_string())?;
    let serial = format!("emulator-{port}");

    let mut args: Vec<String> = vec![
        "-avd".into(),
        name.into(),
        "-port".into(),
        port.to_string(),
        "-no-boot-anim".into(),
        "-gpu".into(),
        settings.gpu_mode.clone(),
        "-accel".into(),
        "auto".into(),
    ];
    if !settings.quick_boot {
        args.push("-no-snapshot".into());
    }
    log("info", format!("Starting {name} on {serial}: emulator {}", args.join(" ")));

    let mut cmd = proc::command(&emu);
    cmd.args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(root) = &sdk.root {
        cmd.env("ANDROID_SDK_ROOT", root);
    }
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start the emulator: {e}"))?;
    if let Some(o) = child.stdout.take() {
        pump(o, log.clone());
    }
    if let Some(e) = child.stderr.take() {
        pump(e, log.clone());
    }

    lock(reg).insert(
        name.to_string(),
        Managed {
            child,
            serial: serial.clone(),
            started: Instant::now(),
            stopping: false,
            booted: false,
        },
    );
    Ok(serial)
}

/// Shuts the device down through the emulator console so Quick Boot can save
/// its snapshot, then force-kills the process if it doesn’t exit in time.
pub fn stop(sdk: &SdkInfo, name: &str, reg: &Registry, log: &Logger) -> Result<(), String> {
    let adb = adb::from_sdk(sdk)?;

    let managed_serial = {
        let mut g = lock(reg);
        g.get_mut(name).map(|m| {
            m.stopping = true;
            m.serial.clone()
        })
    };
    let is_managed = managed_serial.is_some();
    let serial = match managed_serial {
        Some(s) => s,
        None => {
            let devs = adb.devices().unwrap_or_default();
            external(&adb, &devs, reg)
                .get(name)
                .cloned()
                .ok_or_else(|| format!("{name} is not running."))?
        }
    };

    log("info", format!("Stopping {name} ({serial})"));
    if let Err(e) = adb.emu_kill(&serial) {
        log("warn", format!("Graceful shutdown request failed: {e}"));
    }

    let deadline = Instant::now() + Duration::from_secs(25);
    loop {
        let gone = if is_managed {
            match lock(reg).get_mut(name) {
                Some(m) => !matches!(m.child.try_wait(), Ok(None)),
                None => true,
            }
        } else {
            !adb.devices()
                .unwrap_or_default()
                .iter()
                .any(|(s, _)| *s == serial)
        };
        if gone {
            break;
        }
        if Instant::now() > deadline {
            if is_managed {
                if let Some(m) = lock(reg).get_mut(name) {
                    let _ = m.child.kill();
                    let _ = m.child.wait();
                }
                log("warn", format!("{name} did not shut down in time and was force-stopped."));
            }
            break;
        }
        thread::sleep(Duration::from_millis(300));
    }
    lock(reg).remove(name);
    log("info", format!("{name} stopped."));
    Ok(())
}

/// The adb serial of a running device, whether DroidLite started it or not.
pub fn serial_for(adb: &Adb, name: &str, reg: &Registry) -> Option<String> {
    if let Some(m) = lock(reg).get(name) {
        return Some(m.serial.clone());
    }
    let devs = adb.devices().ok()?;
    external(adb, &devs, reg).get(name).cloned()
}

pub fn statuses(sdk: &SdkInfo, names: &[String], reg: &Registry, log: &Logger) -> Vec<DeviceStatus> {
    let adb = match adb::from_sdk(sdk) {
        Ok(a) => a,
        Err(_) => return names.iter().map(|n| stopped(n)).collect(),
    };
    let devs = adb.devices().unwrap_or_default();
    let ext = external(&adb, &devs, reg);
    names
        .iter()
        .map(|n| status_one(&adb, n, &devs, &ext, reg, log))
        .collect()
}

fn status_one(
    adb: &Adb,
    name: &str,
    devs: &[(String, String)],
    ext: &HashMap<String, String>,
    reg: &Registry,
    log: &Logger,
) -> DeviceStatus {
    let mut exited: Option<Option<i32>> = None;
    let mut info: Option<(String, bool)> = None;
    {
        let mut g = lock(reg);
        if let Some(m) = g.get_mut(name) {
            match m.child.try_wait() {
                Ok(Some(st)) => exited = Some(st.code()),
                Ok(None) => info = Some((m.serial.clone(), m.stopping)),
                Err(_) => exited = Some(None),
            }
        }
        if exited.is_some() {
            g.remove(name);
        }
    }
    if let Some(code) = exited {
        match code {
            Some(0) | None => log("info", format!("{name} stopped.")),
            Some(c) => log(
                "error",
                format!("{name} exited unexpectedly (code {c}). Check the emulator output above."),
            ),
        }
        return stopped(name);
    }

    let managed = info.is_some();
    let (serial, stopping) = match info {
        Some((s, st)) => (s, st),
        None => match ext.get(name) {
            Some(s) => (s.clone(), false),
            None => return stopped(name),
        },
    };
    let mk = |state: &str, stage: &str, progress: u8| DeviceStatus {
        name: name.to_string(),
        state: state.to_string(),
        stage: stage.to_string(),
        progress,
        serial: Some(serial.clone()),
        managed,
    };

    if stopping {
        return mk("stopping", "Shutting down", 0);
    }
    let dev_state = devs
        .iter()
        .find(|(s, _)| *s == serial)
        .map(|(_, st)| st.as_str());

    match dev_state {
        None => mk("starting", "Starting the emulator", 8),
        Some("device") => {
            let booted = adb.getprop(&serial, "sys.boot_completed").as_deref() == Some("1");
            if booted {
                if managed {
                    let mut g = lock(reg);
                    if let Some(m) = g.get_mut(name) {
                        if !m.booted {
                            m.booted = true;
                            let secs = m.started.elapsed().as_secs();
                            log("info", format!("{name} is ready ({secs}s)."));
                        }
                    }
                }
                mk("running", "Ready", 100)
            } else if adb.getprop(&serial, "init.svc.bootanim").as_deref() == Some("stopped") {
                mk("starting", "Starting Android", 90)
            } else {
                mk("starting", "Booting Android", 55)
            }
        }
        Some(_) => mk("starting", "Connecting to ADB", 25),
    }
}
