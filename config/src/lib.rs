// SPDX-License-Identifier: GPL-3.0-only

use cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum AppTheme {
    Dark,
    Light,
    System,
}

#[derive(Clone, CosmicConfigEntry, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Config {
    /// Always show OSK
    pub always_shown: bool,
    /// App theme, either system default, dark, or light
    pub app_theme: AppTheme,
    /// Show the OSK on a gamepad shortcut
    pub gamepad_shortcut: bool,
    /// Show the OSK on IME activate
    pub ime_activation: bool,
}

impl Config {
    pub const ID: &'static str = "com.system76.CosmicOSK";
    pub const VERSION: u64 = 1;
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_theme: AppTheme::System,
            always_shown: false,
            gamepad_shortcut: false,
            ime_activation: false,
        }
    }
}
