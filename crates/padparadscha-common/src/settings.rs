//! The app's `[acp].url` and `[server].mode` (§4.1). Directories come from corundum
//! (§2.2); language and theme already belong to its look (§4.1.2).

use serde::Deserialize;
use std::path::Path;

/// Where the game server runs (spec §4.1.3). The server itself carries no mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerMode {
    /// Same process, another thread (desktop, mobile).
    #[default]
    Local,
    /// The URL, connected to.
    Remote,
}

/// The app's two settings, loaded from its config directory (§4.1).
#[derive(Clone, PartialEq, Eq, Debug, Deserialize, bevy::prelude::Resource)]
pub struct Settings {
    /// The agent's URL; local by default.
    pub acp_url: String,
    /// Where the game server runs (§4.1.3).
    pub server_mode: ServerMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            acp_url: default_url(),
            server_mode: ServerMode::default(),
        }
    }
}

#[derive(Deserialize, Default)]
struct File {
    #[serde(default)]
    acp: Acp,
    #[serde(default)]
    server: Server,
}

#[derive(Deserialize)]
struct Acp {
    #[serde(default = "default_url")]
    url: String,
}

impl Default for Acp {
    fn default() -> Self {
        Self { url: default_url() }
    }
}

#[derive(Deserialize, Default)]
struct Server {
    mode: ServerMode,
}

fn default_url() -> String {
    "ws://127.0.0.1:0/acp".to_string()
}

impl Settings {
    /// Read `settings.toml` under `AppDirs::new(app_name).config()` (§2.2).
    pub fn load(dir: &Path) -> Self {
        let text = std::fs::read_to_string(dir.join("settings.toml")).unwrap_or_default();
        let file: File = toml::from_str(&text).unwrap_or_default();
        Self {
            acp_url: file.acp.url,
            server_mode: file.server.mode,
        }
    }
}
