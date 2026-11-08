use directories::{ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub output_directory: PathBuf,
    pub workers: usize,
    pub auto_decrypt: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        let output_directory = UserDirs::new()
            .and_then(|directories| directories.download_dir().map(PathBuf::from))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
            .join("Endao Books");
        Self {
            output_directory,
            workers: 4,
            auto_decrypt: true,
        }
    }
}

pub fn load() -> AppSettings {
    let Some(path) = settings_path() else {
        return AppSettings::default();
    };
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save(settings: &AppSettings) {
    let Some(path) = settings_path() else { return };
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
        let _ = fs::write(path, bytes);
    }
}

fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("com", "Endao", "Endao Desktop")
        .map(|directories| directories.config_dir().join("settings.json"))
}
