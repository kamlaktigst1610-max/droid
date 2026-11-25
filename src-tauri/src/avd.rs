//! Android Virtual Device handling: list existing AVDs, create new ones with
//! lightweight settings (avdmanager + config.ini overrides).

use crate::proc::{self, Logger};
use crate::sdk::{self, SdkInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Serialize, Clone, Debug)]
pub struct AvdInfo {
    pub name: String,
    pub dir: String,
    pub image: String,
    pub abi: String,
    pub api: Option<u32>,
    pub android: String,
    pub play_store: bool,
    pub cpus: u32,
    pub ram_mb: u32,
    pub width: u32,
    pub height: u32,
    pub density: u32,
}

#[derive(Deserialize, Clone, Debug)]
pub struct CreateRequest {
    pub name: String,
    pub image_id: String,
    pub cpus: u32,
    pub ram_mb: u32,
    pub width: u32,
    pub height: u32,
    pub density: u32,
}

pub fn avd_home() -> Option<PathBuf> {
    if let Some(p) = sdk::env_path("ANDROID_AVD_HOME") {
        return Some(p);
    }
    if let Some(p) = sdk::env_path("ANDROID_USER_HOME") {
        return Some(p.join("avd"));
    }
    sdk::home_dir().map(|h| h.join(".android").join("avd"))
}

pub fn parse_ini(text: &str) -> HashMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (k, v) = l.split_once('=')?;
            Some((k.trim().to_string(), v.trim().to_string()))
        })
        .collect()
}

/// Reads values such as `1536`, `1536MB` or `2G` as megabytes.
pub fn parse_mb(v: &str) -> u32 {
    let v = v.trim();
    let digits: String = v.chars().take_while(|c| c.is_ascii_digit()).collect();
    let n: u32 = digits.parse().unwrap_or(0);
    if v.to_ascii_uppercase().ends_with('G') || v.to_ascii_uppercase().ends_with("GB") {
        n * 1024
    } else {
        n
    }
}

fn num(map: &HashMap<String, String>, key: &str) -> u32 {
    map.get(key)
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

pub fn list_avds() -> Vec<AvdInfo> {
    let Some(home) = avd_home() else {
        return vec![];
    };
    let Ok(rd) = fs::read_dir(&home) else {
        return vec![];
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "ini").unwrap_or(true) {
            continue;
        }
        let Some(name) = p.file_stem().map(|n| n.to_string_lossy().into_owned()) else {
            continue;
        };
        let ini = parse_ini(&fs::read_to_string(&p).unwrap_or_default());
        let mut dir = ini
            .get("path")
            .map(PathBuf::from)
            .filter(|d| d.is_dir())
            .unwrap_or_else(|| home.join(format!("{name}.avd")));
        if !dir.is_dir() {
            dir = home.join(format!("{name}.avd"));
        }
        let Ok(cfg_text) = fs::read_to_string(dir.join("config.ini")) else {
            continue;
        };
        let cfg = parse_ini(&cfg_text);
        let image = cfg.get("image.sysdir.1").cloned().unwrap_or_default();
        let api = image
            .split(['/', '\\'])
            .find_map(|seg| seg.strip_prefix("android-").and_then(|n| n.parse::<u32>().ok()));
        out.push(AvdInfo {
            name,
            dir: dir.to_string_lossy().into_owned(),
            abi: cfg.get("abi.type").cloned().unwrap_or_default(),
            android: api.map(sdk::android_name).unwrap_or_else(|| "Unknown".to_string()),
            api,
            play_store: cfg
                .get("PlayStore.enabled")
                .map(|v| v == "true" || v == "yes")
                .unwrap_or(false),
            cpus: num(&cfg, "hw.cpu.ncore").max(1),
            ram_mb: cfg.get("hw.ramSize").map(|v| parse_mb(v)).unwrap_or(0),
            width: num(&cfg, "hw.lcd.width"),
            height: num(&cfg, "hw.lcd.height"),
            density: num(&cfg, "hw.lcd.density"),
            image,
        });
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

/// Replaces existing `key=value` lines and appends any keys that were missing.
pub fn patch_ini(text: &str, pairs: &[(&str, String)]) -> String {
    let mut done = vec![false; pairs.len()];
    let mut out = String::new();
    for line in text.lines() {
        let key = line.split('=').next().unwrap_or("").trim();
        if let Some(i) = pairs.iter().position(|(k, _)| *k == key) {
            out.push_str(&format!("{}={}\n", pairs[i].0, pairs[i].1));
            done[i] = true;
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    for (i, (k, v)) in pairs.iter().enumerate() {
        if !done[i] {
            out.push_str(&format!("{k}={v}\n"));
        }
    }
    out
}

fn validate(req: &CreateRequest) -> Result<(), String> {
    let name_ok = !req.name.is_empty()
        && req.name.len() <= 40
        && req
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
    if !name_ok {
        return Err("Use 1–40 letters, numbers, dots, dashes or underscores for the name.".to_string());
    }
    if !(1..=16).contains(&req.cpus) {
        return Err("CPU cores must be between 1 and 16.".to_string());
    }
    if !(512..=16384).contains(&req.ram_mb) {
        return Err("RAM must be between 512 and 16384 MB.".to_string());
    }
    if !(240..=4096).contains(&req.width) || !(240..=4096).contains(&req.height) {
        return Err("Screen width and height must be between 240 and 4096 pixels.".to_string());
    }
    if !(100..=640).contains(&req.density) {
        return Err("Screen density must be between 100 and 640 dpi.".to_string());
    }
    Ok(())
}

pub fn create(sdk: &SdkInfo, req: &CreateRequest, log: &Logger) -> Result<String, String> {
    validate(req)?;
    let avdm = sdk.avdmanager.as_ref().ok_or_else(|| {
        "avdmanager was not found. Install “Android SDK Command-line Tools” in the SDK Manager."
            .to_string()
    })?;
    if !sdk.system_images.iter().any(|i| i.id == req.image_id) {
        return Err("That system image isn’t installed.".to_string());
    }
    if list_avds().iter().any(|a| a.name == req.name) {
        return Err(format!("A device named “{}” already exists.", req.name));
    }

    log("info", format!("Creating {} from {}", req.name, req.image_id));
    let mut envs: Vec<(&str, &str)> = Vec::new();
    if let Some(r) = sdk.root.as_deref() {
        envs.push(("ANDROID_SDK_ROOT", r));
    }
    let out = proc::run_env(
        Path::new(avdm),
        &[
            "create",
            "avd",
            "--force",
            "--name",
            req.name.as_str(),
            "--package",
            req.image_id.as_str(),
        ],
        Some("no\n"), // answers “Do you wish to create a custom hardware profile?”
        Duration::from_secs(120),
        &envs,
    )?;
    if !out.success() {
        return Err(format!("avdmanager failed: {}", out.combined()));
    }

    let created = list_avds()
        .into_iter()
        .find(|a| a.name == req.name)
        .ok_or_else(|| "avdmanager finished but the device could not be found.".to_string())?;
    let cfg_path = Path::new(&created.dir).join("config.ini");
    let text = fs::read_to_string(&cfg_path).map_err(|e| format!("Could not read config.ini: {e}"))?;
    let pairs: Vec<(&str, String)> = vec![
        ("hw.cpu.ncore", req.cpus.to_string()),
        ("hw.ramSize", req.ram_mb.to_string()),
        ("hw.lcd.width", req.width.to_string()),
        ("hw.lcd.height", req.height.to_string()),
        ("hw.lcd.density", req.density.to_string()),
        ("disk.dataPartition.size", "4G".to_string()),
        ("hw.gpu.enabled", "yes".to_string()),
        ("hw.gpu.mode", "auto".to_string()),
        ("hw.keyboard", "yes".to_string()),
        ("showDeviceFrame", "no".to_string()),
        ("fastboot.forceColdBoot", "no".to_string()),
        ("fastboot.forceFastBoot", "yes".to_string()),
    ];
    fs::write(&cfg_path, patch_ini(&text, &pairs))
        .map_err(|e| format!("Could not write config.ini: {e}"))?;

    log(
        "info",
        format!(
            "Created {}: {} CPU, {} MB RAM, {}×{}",
            req.name, req.cpus, req.ram_mb, req.width, req.height
        ),
    );
    Ok(req.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patches_and_appends() {
        let out = patch_ini(
            "hw.ramSize=2048\nfoo=bar\n",
            &[("hw.ramSize", "1536".to_string()), ("hw.cpu.ncore", "2".to_string())],
        );
        assert!(out.contains("hw.ramSize=1536\n"));
        assert!(out.contains("foo=bar\n"));
        assert!(out.contains("hw.cpu.ncore=2\n"));
        assert!(!out.contains("2048"));
    }

    #[test]
    fn parses_memory_sizes() {
        assert_eq!(parse_mb("1536"), 1536);
        assert_eq!(parse_mb("1536MB"), 1536);
        assert_eq!(parse_mb("2G"), 2048);
    }
}
