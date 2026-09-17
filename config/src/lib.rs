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
    /// Show function row and system keys
    pub function_row: bool,
    /// Show the OSK on a gamepad shortcut
    pub gamepad_shortcut: bool,
    /// Show the OSK on IME activate
    pub ime_activation: bool,
    /// Show numpad
    pub numpad: bool,
}

impl Config {
    pub const ID: &'static str = "com.system76.CosmicOSK";
    pub const VERSION: u64 = 1;

    pub fn handler() -> Result<cosmic_config::Config, cosmic_config::Error> {
        // Ues state as many settings may be machine specific
        cosmic_config::Config::new_state(Self::ID, Self::VERSION)
    }

    #[cfg(feature = "subscription")]
    pub fn subscription() -> iced_futures::Subscription<cosmic_config::Update<Self>> {
        struct ConfigSubscription;
        // Ues state as many settings may be machine specific
        cosmic_config::config_state_subscription::<_, Config>(
            std::any::TypeId::of::<ConfigSubscription>(),
            Self::ID.into(),
            Self::VERSION,
        )
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_theme: AppTheme::System,
            always_shown: false,
            function_row: true,
            gamepad_shortcut: false,
            ime_activation: false,
            numpad: false,
        }
    }
}
