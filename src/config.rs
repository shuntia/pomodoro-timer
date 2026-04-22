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

fn default_work_time() -> u64 {
    25 * 60
}
fn default_break_time() -> u64 {
    5 * 60
}
fn default_true() -> bool {
    true
}
fn default_main_music_dir() -> String {
    dirs::home_dir()
        .map(|h| h.join(".timer/music/main").to_string_lossy().into_owned())
        .unwrap_or_default()
}
fn default_break_music_dir() -> String {
    dirs::home_dir()
        .map(|h| h.join(".timer/music/break").to_string_lossy().into_owned())
        .unwrap_or_default()
}
fn default_accent_color() -> [f32; 3] {
    [0.95, 0.48, 0.12]
}
fn default_break_color() -> [f32; 3] {
    [0.25, 0.85, 0.45]
}
fn default_blur_intensity() -> f32 {
    18.0
}
fn default_font_size_scale() -> f32 {
    1.0
}
fn default_mode_font_size_scale() -> f32 {
    1.0
}
fn default_mode_font_color() -> [f32; 3] {
    [1.0, 1.0, 1.0]
}
fn default_mode_font_opacity() -> f32 {
    0.55
}
fn default_ring_thickness_scale() -> f32 {
    1.0
}
fn default_ring_bg_opacity() -> f32 {
    0.10
}
fn default_bg_tint() -> [f32; 3] {
    [0.0, 0.0, 0.0]
}
fn default_bg_tint_strength() -> f32 {
    0.47
}
fn default_timer_opacity() -> f32 {
    1.0
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TimerFont {
    Default,
    Monospace,
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Custom,
}

impl Default for TimerFont {
    fn default() -> Self {
        TimerFont::Default
    }
}

impl std::fmt::Display for TimerFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

impl TimerFont {
    pub fn label(self) -> &'static str {
        match self {
            TimerFont::Default => "Default",
            TimerFont::Monospace => "Monospace",
            TimerFont::Serif => "Serif",
            TimerFont::SansSerif => "Sans-Serif",
            TimerFont::Cursive => "Cursive",
            TimerFont::Fantasy => "Fantasy",
            TimerFont::Custom => "Custom",
        }
    }

    pub fn all() -> &'static [TimerFont] {
        &[
            TimerFont::Default,
            TimerFont::Monospace,
            TimerFont::Serif,
            TimerFont::SansSerif,
            TimerFont::Cursive,
            TimerFont::Fantasy,
            TimerFont::Custom,
        ]
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
    #[serde(default = "default_main_music_dir")]
    pub main_music_dir: String,
    #[serde(default = "default_break_music_dir")]
    pub break_music_dir: String,
    #[serde(default = "default_accent_color")]
    pub accent_color: [f32; 3],
    #[serde(default = "default_break_color")]
    pub break_color: [f32; 3],
    #[serde(default)]
    pub timer_font: TimerFont,
    /// Font family name for Custom timer font (must be system-installed).
    #[serde(default)]
    pub custom_font_name: String,
    /// Scale multiplier for the mode label font (0.3–3.0, default 1.0).
    #[serde(default = "default_mode_font_size_scale")]
    pub mode_font_size_scale: f32,
    /// RGB color for the mode label ("Work" / "Break").
    #[serde(default = "default_mode_font_color")]
    pub mode_font_color: [f32; 3],
    /// Opacity of the mode label (0.0–1.0, default 0.55).
    #[serde(default = "default_mode_font_opacity")]
    pub mode_font_opacity: f32,
    /// Gaussian blur sigma for static wallpaper and album-art backgrounds.
    /// Also controls the number of GStreamer gleffects passes for video blur.
    #[serde(default = "default_blur_intensity")]
    pub blur_intensity: f32,
    /// Scale multiplier for the timer font (0.3 – 3.0, default 1.0).
    #[serde(default = "default_font_size_scale")]
    pub font_size_scale: f32,
    /// Scale multiplier for the ring stroke width (0.1 – 5.0, default 1.0).
    #[serde(default = "default_ring_thickness_scale")]
    pub ring_thickness_scale: f32,
    /// Opacity of the background (unlit) portion of the ring (0.0 – 1.0, default 0.10).
    #[serde(default = "default_ring_bg_opacity")]
    pub ring_bg_opacity: f32,
    /// RGB tint color applied over the background image/video.
    #[serde(default = "default_bg_tint")]
    pub bg_tint: [f32; 3],
    /// Opacity of the tint overlay (0.0 = off, 1.0 = fully opaque, default 0.47).
    #[serde(default = "default_bg_tint_strength")]
    pub bg_tint_strength: f32,
    #[serde(default)]
    pub countdown_sound_path: String,
    #[serde(default)]
    pub bell_sound_path: String,
    /// Opacity of the timer digits (0.0–1.0, default 1.0).
    #[serde(default = "default_timer_opacity")]
    pub timer_opacity: f32,
    /// Whether shuffle is enabled for music playback.
    #[serde(default)]
    pub shuffle_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work_time: default_work_time(),
            break_time: default_break_time(),
            music_enabled: default_true(),
            wallpaper_type: WallpaperType::None,
            wallpaper_path: String::new(),
            main_music_dir: default_main_music_dir(),
            break_music_dir: default_break_music_dir(),
            accent_color: default_accent_color(),
            break_color: default_break_color(),
            timer_font: TimerFont::default(),
            custom_font_name: String::new(),
            mode_font_size_scale: default_mode_font_size_scale(),
            mode_font_color: default_mode_font_color(),
            mode_font_opacity: default_mode_font_opacity(),
            blur_intensity: default_blur_intensity(),
            font_size_scale: default_font_size_scale(),
            ring_thickness_scale: default_ring_thickness_scale(),
            ring_bg_opacity: default_ring_bg_opacity(),
            bg_tint: default_bg_tint(),
            bg_tint_strength: default_bg_tint_strength(),
            countdown_sound_path: String::new(),
            bell_sound_path: String::new(),
            timer_opacity: default_timer_opacity(),
            shuffle_enabled: false,
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
