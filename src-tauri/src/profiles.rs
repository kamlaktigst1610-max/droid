use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct DeviceProfile {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub cpus: u32,
    pub ram_mb: u32,
    pub width: u32,
    pub height: u32,
    pub density: u32,
}

/// The first profile is the default.
pub fn all() -> Vec<DeviceProfile> {
    vec![
        DeviceProfile {
            id: "lite",
            label: "Lite phone",
            description: "Default. Small screen, low memory use.",
            cpus: 2,
            ram_mb: 1536,
            width: 480,
            height: 854,
            density: 240,
        },
        DeviceProfile {
            id: "minimal",
            label: "Minimal phone",
            description: "For very old PCs or simple apps.",
            cpus: 1,
            ram_mb: 1024,
            width: 480,
            height: 800,
            density: 240,
        },
        DeviceProfile {
            id: "standard",
            label: "Standard phone",
            description: "720p screen for apps that need more room.",
            cpus: 2,
            ram_mb: 2048,
            width: 720,
            height: 1280,
            density: 320,
        },
        DeviceProfile {
            id: "tablet",
            label: "Small tablet",
            description: "Tablet layout at 800×1280.",
            cpus: 2,
            ram_mb: 2048,
            width: 800,
            height: 1280,
            density: 213,
        },
    ]
}
