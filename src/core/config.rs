//! Configuration and path management for Tactica
//!
//! Handles application settings persistence and data directory paths.
//! Settings are stored in XDG_CONFIG_HOME (e.g. ~/.config/tactica/settings.json)
//! Data is stored in XDG_DATA_HOME (e.g. ~/.local/share/tactica/)

use crate::storage::{Storage, write_atomic};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

// --- Settings ---

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum LayoutMode {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum AnimationSpeed {
    Off,
    Fast,
    #[default]
    Normal,
    Slow,
}

impl AnimationSpeed {
    /// Progress increment per 16ms tick (60 FPS).
    /// Higher = faster animation.
    pub fn progress_per_tick(self) -> f32 {
        match self {
            AnimationSpeed::Off => 1.0,
            AnimationSpeed::Fast => 0.35,
            AnimationSpeed::Normal => 0.2,
            AnimationSpeed::Slow => 0.1,
        }
    }

    pub fn is_off(self) -> bool {
        self == AnimationSpeed::Off
    }

    pub fn cycle(self) -> Self {
        match self {
            AnimationSpeed::Off => AnimationSpeed::Fast,
            AnimationSpeed::Fast => AnimationSpeed::Normal,
            AnimationSpeed::Normal => AnimationSpeed::Slow,
            AnimationSpeed::Slow => AnimationSpeed::Off,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AnimationSpeed::Off => "Off",
            AnimationSpeed::Fast => "Fast",
            AnimationSpeed::Normal => "Normal",
            AnimationSpeed::Slow => "Slow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum BoardTheme {
    Blue,
    Green,
    #[default]
    Brown,
}

impl BoardTheme {
    pub fn label(self) -> &'static str {
        match self {
            BoardTheme::Blue => "Blue",
            BoardTheme::Green => "Green",
            BoardTheme::Brown => "Brown",
        }
    }

    pub fn cycle(self) -> Self {
        match self {
            BoardTheme::Blue => BoardTheme::Green,
            BoardTheme::Green => BoardTheme::Brown,
            BoardTheme::Brown => BoardTheme::Blue,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub library_root: Option<PathBuf>,
    pub board_size: f32,
    pub ui_scale: f32,
    #[serde(default)]
    pub help_expanded: bool,
    #[serde(default)]
    pub layout_mode: LayoutMode,
    #[serde(default = "default_auto_layout")]
    pub auto_layout: bool,
    #[serde(default)]
    pub board_theme: BoardTheme,
    #[serde(default = "default_show_eval_bar")]
    pub show_eval_bar: bool,
    #[serde(default)]
    pub animation_speed: AnimationSpeed,
    #[serde(default)]
    pub recent_items: Vec<PathBuf>,
    #[serde(default)]
    pub library_sidebar_open: bool,
    #[serde(default)]
    pub engine: EngineSettings,
}

fn default_auto_layout() -> bool {
    true
}

/// Engine-specific settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSettings {
    /// Whether the engine is enabled
    pub enabled: bool,
    /// Number of analysis lines (MultiPV), 1-5
    pub multi_pv: u32,
    /// Maximum search depth (None = infinite)
    pub max_depth: Option<u32>,
    /// Number of CPU threads for the engine
    pub threads: u32,
    /// Hash table size in MB
    pub hash_mb: u32,
}

impl Default for EngineSettings {
    fn default() -> Self {
        // Detect available cores, use half (min 1, max 8)
        let available = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(2);
        let default_threads = (available / 2).clamp(1, 8) as u32;

        Self {
            enabled: true,
            multi_pv: 3,
            max_depth: None, // Infinite by default
            threads: default_threads,
            hash_mb: 256,
        }
    }
}

impl EngineSettings {
    /// Available thread options based on system
    pub fn available_thread_options() -> Vec<u32> {
        let available = std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(2) as u32;
        let max = available.min(16);
        vec![1, 2, 4, 8, 16]
            .into_iter()
            .filter(|&t| t <= max)
            .collect()
    }

    /// Available hash size options
    pub fn available_hash_options() -> Vec<u32> {
        vec![64, 128, 256, 512, 1024]
    }

    /// Available depth options (None = infinite)
    pub fn available_depth_options() -> Vec<Option<u32>> {
        vec![Some(15), Some(20), Some(25), Some(30), None]
    }

    /// Format depth for display
    pub fn format_depth(depth: Option<u32>) -> &'static str {
        match depth {
            Some(15) => "15",
            Some(20) => "20",
            Some(25) => "25",
            Some(30) => "30",
            None => "Max",
            _ => "?",
        }
    }

    /// Get max available threads on this system
    pub fn max_threads() -> u32 {
        std::thread::available_parallelism()
            .map(|p| p.get() as u32)
            .unwrap_or(2)
    }
}

fn default_show_eval_bar() -> bool {
    true
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            library_root: None,
            board_size: 480.0,
            ui_scale: 1.0,
            help_expanded: false,
            layout_mode: LayoutMode::Horizontal,
            auto_layout: true,
            board_theme: BoardTheme::default(),
            show_eval_bar: true,
            animation_speed: AnimationSpeed::Normal,
            recent_items: Vec::new(),
            library_sidebar_open: true,
            engine: EngineSettings::default(),
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(content) = fs::read_to_string(&path)
            && let Ok(settings) = serde_json::from_str(&content)
        {
            return settings;
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        write_atomic(&path, json.as_bytes())
    }

    /// Add a path to the recent items list
    pub fn add_recent_item(&mut self, path: PathBuf) {
        self.recent_items.retain(|p| p != &path);
        self.recent_items.insert(0, path);
        self.recent_items.truncate(10);
    }

    fn path() -> PathBuf {
        Storage::settings_path()
    }

    pub fn storage(&self) -> Storage {
        Storage::new(
            self.library_root
                .clone()
                .unwrap_or_else(Storage::default_root),
        )
    }

    pub fn library_root(&self) -> PathBuf {
        self.storage().root().to_path_buf()
    }
}

// --- Paths ---

/// Get the data directory (~/.local/share/tactica/)
/// Creates it if it doesn't exist.
pub fn data_dir() -> PathBuf {
    let path = AppSettings::load().library_root();
    let _ = fs::create_dir_all(&path);
    path
}

/// Get the studies directory (for Study PGN files)
pub fn studies_dir() -> PathBuf {
    let path = AppSettings::load().storage().studies_dir();
    let _ = fs::create_dir_all(&path);
    path
}

/// Get the reviews directory (for Game Review PGN + sidecar files)
pub fn reviews_dir() -> PathBuf {
    let path = AppSettings::load().storage().reviews_dir();
    let _ = fs::create_dir_all(&path);
    path
}
