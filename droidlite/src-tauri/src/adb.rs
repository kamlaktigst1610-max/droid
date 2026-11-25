use crate::proc::{self, RunOutput};
use crate::sdk::SdkInfo;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static SERVER_STARTED: AtomicBool = AtomicBool::new(false);

pub struct Adb {
    path: PathBuf,
}

pub fn from_sdk(sdk: &SdkInfo) -> Result<Adb, String> {
    match &sdk.adb {
        Some(p) => Ok(Adb { path: PathBuf::from(p) }),
        None => Err("adb was not found. Install “platform-tools” in the SDK Manager.".to_string()),
    }
}

/// Parses the output of `adb devices` into (serial, state) pairs.
pub fn parse_devices(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('*') && !l.starts_with("List of"))
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            Some((it.next()?.to_string(), it.next()?.to_string()))
        })
        .collect()
}

impl Adb {
    fn run(&self, args: &[&str], secs: u64) -> Result<RunOutput, String> {
        proc::run(&self.path, args, None, Duration::from_secs(secs))
    }

    /// Starts the adb server once per app session so later calls stay quick.
    pub fn ensure_server(&self) {
        if !SERVER_STARTED.swap(true, Ordering::SeqCst) {
            let _ = self.run(&["start-server"], 15);
        }
    }

    pub fn devices(&self) -> Result<Vec<(String, String)>, String> {
        self.ensure_server();
        let out = self.run(&["devices"], 8)?;
        Ok(parse_devices(&out.stdout))
    }

    pub fn getprop(&self, serial: &str, key: &str) -> Option<String> {
        let out = self.run(&["-s", serial, "shell", "getprop", key], 5).ok()?;
        let v = out.stdout.trim().to_string();
        (out.success() && !v.is_empty()).then_some(v)
    }

    /// Asks the emulator to shut down (this also saves the Quick Boot snapshot).
    pub fn emu_kill(&self, serial: &str) -> Result<(), String> {
        let out = self.run(&["-s", serial, "emu", "kill"], 10)?;
        if out.success() {
            Ok(())
        } else {
            Err(out.combined())
        }
    }

    /// The AVD name behind an `emulator-NNNN` serial, via the emulator console.
    pub fn avd_name(&self, serial: &str) -> Option<String> {
        let out = self.run(&["-s", serial, "emu", "avd", "name"], 5).ok()?;
        out.stdout
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && *l != "OK")
            .map(str::to_string)
    }

    pub fn install(&self, serial: &str, apk: &str) -> Result<String, String> {
        let p = Path::new(apk);
        if !p.is_file() {
            return Err(format!("File not found: {apk}"));
        }
        let is_apk = p
            .extension()
            .map(|e| e.to_string_lossy().eq_ignore_ascii_case("apk"))
            .unwrap_or(false);
        if !is_apk {
            return Err("Choose an .apk file.".to_string());
        }
        let out = self.run(&["-s", serial, "install", "-r", "-t", apk], 300)?;
        let text = out.combined();
        if out.success() && text.contains("Success") {
            Ok("Installed.".to_string())
        } else {
            Err(explain_install_failure(&text))
        }
    }
}

pub fn explain_install_failure(text: &str) -> String {
    let hint = if text.contains("INSTALL_FAILED_NO_MATCHING_ABIS") {
        "\nThis app has no x86_64 code. Use an APK built for x86/x86_64, or a universal APK."
    } else if text.contains("INSTALL_FAILED_OLDER_SDK") {
        "\nThis app needs a newer Android version than this device runs."
    } else if text.contains("INSTALL_FAILED_INSUFFICIENT_STORAGE") {
        "\nThe device is out of storage."
    } else {
        ""
    };
    format!("{}{}", text.trim(), hint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_device_list() {
        let t = "* daemon started successfully\nList of devices attached\nemulator-5554\tdevice\nemulator-5556\toffline\n\n";
        let d = parse_devices(t);
        assert_eq!(d.len(), 2);
        assert_eq!(d[0], ("emulator-5554".to_string(), "device".to_string()));
        assert_eq!(d[1].1, "offline");
    }
}
