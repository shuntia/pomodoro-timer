use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WallpaperType {
    None,
    Static,
}

impl Default for WallpaperType {
    fn default() -> Self {
        WallpaperType::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_work_time")]
    pub work_time: u64,
    #[serde(default = "default_break_time")]
    pub break_time: u64,
    #[serde(default = "default_true")]
    pub music_enabled: bool,
    #[serde(default)]
    pub wallpaper_type: WallpaperType,
    #[serde(default)]
    pub wallpaper_path: String,
}

fn default_work_time() -> u64 { 25 * 60 }
fn default_break_time() -> u64 { 5 * 60 }
fn default_true() -> bool { true }

impl Default for Config {
    fn default() -> Self {
        Self {
            work_time: default_work_time(),
            break_time: default_break_time(),
            music_enabled: default_true(),
            wallpaper_type: WallpaperType::None,
            wallpaper_path: String::new(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = Self::path();
        if path.exists() {
            match std::fs::read_to_string(&path) {
                Ok(s) => toml::from_str(&s).unwrap_or_default(),
                Err(_) => Config::default(),
            }
        } else {
            Config::default()
        }
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }

    pub fn path() -> PathBuf {
        dirs::home_dir().unwrap().join(".timer/config.toml")
    }
}
