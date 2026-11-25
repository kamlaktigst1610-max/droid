use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const GPU_MODES: [&str; 4] = ["auto", "host", "swiftshader_indirect", "angle_indirect"];

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Overrides SDK auto-detection when set.
    pub sdk_path: Option<String>,
    pub gpu_mode: String,
    pub quick_boot: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            sdk_path: None,
            gpu_mode: "auto".to_string(),
            quick_boot: true,
        }
    }
}

pub fn load(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, s: &Settings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| format!("Could not save settings: {e}"))
}
