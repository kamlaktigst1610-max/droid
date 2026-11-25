//! Android SDK detection: root folder, tool binaries, installed system images.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

pub fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

pub fn bat(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.bat")
    } else {
        name.to_string()
    }
}

pub fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn home_dir() -> Option<PathBuf> {
    env_path("USERPROFILE").or_else(|| env_path("HOME"))
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[derive(Serialize, Clone, Debug)]
pub struct SystemImage {
    /// Package id, e.g. `system-images;android-30;default;x86_64`.
    pub id: String,
    pub api: u32,
    pub android: String,
    pub variant: String,
    pub abi: String,
    pub path: String,
    pub recommended: bool,
}

#[derive(Serialize, Clone, Debug)]
pub struct SdkInfo {
    pub found: bool,
    pub root: Option<String>,
    pub source: String,
    pub emulator: Option<String>,
    pub adb: Option<String>,
    pub avdmanager: Option<String>,
    pub licenses_accepted: bool,
    pub system_images: Vec<SystemImage>,
    pub problems: Vec<String>,
}

pub fn android_name(api: u32) -> String {
    let v = match api {
        26 => "8.0",
        27 => "8.1",
        28 => "9",
        29 => "10",
        30 => "11",
        31 => "12",
        32 => "12L",
        33 => "13",
        34 => "14",
        35 => "15",
        36 => "16",
        _ => return format!("API {api}"),
    };
    format!("Android {v}")
}

fn candidate_roots() -> Vec<(PathBuf, &'static str)> {
    let mut v = Vec::new();
    if let Some(p) = env_path("ANDROID_SDK_ROOT") {
        v.push((p, "ANDROID_SDK_ROOT"));
    }
    if let Some(p) = env_path("ANDROID_HOME") {
        v.push((p, "ANDROID_HOME"));
    }
    if let Some(p) = env_path("LOCALAPPDATA") {
        v.push((p.join("Android").join("Sdk"), "default location"));
    }
    if let Some(h) = home_dir() {
        v.push((h.join("Android").join("Sdk"), "default location"));
        v.push((h.join("Library").join("Android").join("sdk"), "default location"));
    }
    v
}

fn looks_like_sdk(root: &Path) -> bool {
    root.join("emulator").join(exe("emulator")).is_file()
        || root.join("platform-tools").join(exe("adb")).is_file()
        || root.join("cmdline-tools").is_dir()
}

fn find_avdmanager(root: &Path) -> Option<PathBuf> {
    let name = bat("avdmanager");
    let latest = root.join("cmdline-tools").join("latest").join("bin").join(&name);
    if latest.is_file() {
        return Some(latest);
    }
    if let Ok(rd) = fs::read_dir(root.join("cmdline-tools")) {
        for e in rd.flatten() {
            let p = e.path().join("bin").join(&name);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let legacy = root.join("tools").join("bin").join(&name);
    legacy.is_file().then_some(legacy)
}

fn subdirs(p: &Path) -> Vec<PathBuf> {
    fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Looks in `<sdk>/system-images/android-<api>/<variant>/<abi>/`.
pub fn scan_system_images(root: &Path) -> Vec<SystemImage> {
    let mut out = Vec::new();
    for api_dir in subdirs(&root.join("system-images")) {
        let api_name = file_name(&api_dir);
        let Some(api) = api_name
            .strip_prefix("android-")
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue; // preview images such as android-VanillaIceCream
        };
        for variant_dir in subdirs(&api_dir) {
            let variant = file_name(&variant_dir);
            for abi_dir in subdirs(&variant_dir) {
                let abi = file_name(&abi_dir);
                if !abi_dir.join("source.properties").is_file()
                    && !abi_dir.join("system.img").is_file()
                {
                    continue;
                }
                let recommended = (api == 30 || api == 31)
                    && abi == "x86_64"
                    && (variant == "default" || variant == "google_apis");
                out.push(SystemImage {
                    id: format!("system-images;{api_name};{variant};{abi}"),
                    api,
                    android: android_name(api),
                    variant: variant.clone(),
                    abi,
                    path: s(&abi_dir),
                    recommended,
                });
            }
        }
    }
    out.sort_by_key(|i| {
        (
            !i.recommended,
            i.variant != "default",
            i.abi != "x86_64",
            i.api,
        )
    });
    out
}

pub fn detect(override_path: Option<&str>) -> SdkInfo {
    let mut problems: Vec<String> = Vec::new();
    let mut chosen: Option<(PathBuf, String)> = None;

    if let Some(p) = override_path.map(str::trim).filter(|p| !p.is_empty()) {
        if looks_like_sdk(Path::new(p)) {
            chosen = Some((PathBuf::from(p), "Settings".to_string()));
        } else {
            problems.push(format!(
                "The SDK folder set in Settings doesn’t look like an Android SDK: {p}"
            ));
        }
    }
    if chosen.is_none() {
        for (p, src) in candidate_roots() {
            if looks_like_sdk(&p) {
                chosen = Some((p, src.to_string()));
                break;
            }
        }
    }

    let Some((root, source)) = chosen else {
        problems.push(
            "No Android SDK found. Install Android Studio (or the command-line tools), then set the SDK folder in Settings."
                .to_string(),
        );
        return SdkInfo {
            found: false,
            root: None,
            source: "none".to_string(),
            emulator: None,
            adb: None,
            avdmanager: None,
            licenses_accepted: false,
            system_images: vec![],
            problems,
        };
    };

    let emulator = root.join("emulator").join(exe("emulator"));
    let adb = root.join("platform-tools").join(exe("adb"));
    let avdmanager = find_avdmanager(&root);
    let licenses_accepted = root.join("licenses").join("android-sdk-license").is_file();
    let system_images = scan_system_images(&root);

    let emulator = emulator.is_file().then(|| s(&emulator));
    let adb = adb.is_file().then(|| s(&adb));

    if emulator.is_none() {
        problems.push("Android Emulator is missing. Run: sdkmanager \"emulator\"".to_string());
    }
    if adb.is_none() {
        problems.push("Platform-tools (adb) are missing. Run: sdkmanager \"platform-tools\"".to_string());
    }
    if avdmanager.is_none() {
        problems.push(
            "avdmanager is missing. Install “Android SDK Command-line Tools (latest)” in Android Studio’s SDK Manager."
                .to_string(),
        );
    }
    if !licenses_accepted {
        problems.push("SDK licenses are not accepted yet. Run: sdkmanager --licenses".to_string());
    }
    if system_images.is_empty() {
        problems.push(
            "No system images installed. Run: sdkmanager \"system-images;android-30;default;x86_64\""
                .to_string(),
        );
    } else if !system_images.iter().any(|i| i.recommended) {
        problems.push(
            "No Android 11/12 x86_64 image without Google Play found. Run: sdkmanager \"system-images;android-30;default;x86_64\""
                .to_string(),
        );
    }

    SdkInfo {
        found: true,
        root: Some(s(&root)),
        source,
        emulator,
        adb,
        avdmanager: avdmanager.map(|p| s(&p)),
        licenses_accepted,
        system_images,
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_android_versions() {
        assert_eq!(android_name(30), "Android 11");
        assert_eq!(android_name(99), "API 99");
    }

    #[test]
    fn missing_override_is_reported() {
        let info = detect(Some("/definitely/not/an/sdk"));
        assert!(info.problems.iter().any(|p| p.contains("Settings")));
    }
}
