//! The overall flow shared by classics and the sandbox, as one bevy plugin (spec §4).
//! Task 9 extends this foundation with the ACP session and chat panel.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use corundum_framework::gui::{i18n, look, look::Look, store};

use crate::Settings;

/// This milestone's API version, sent on the handshake and checked against the peer's.
pub const API_VERSION: &str = "1";

/// The handshake's refusal (§10), carrying both versions for a clear message.
#[derive(Debug, thiserror::Error)]
#[error("the API versions do not match: this app speaks {expected}, the peer answered {found}")]
pub struct VersionMismatch {
    pub expected: &'static str,
    pub found: String,
}

/// The API version a peer answered with, or the refusal (§10).
pub fn check_api_version(found: &str) -> Result<(), VersionMismatch> {
    (found == API_VERSION)
        .then_some(())
        .ok_or_else(|| VersionMismatch {
            expected: API_VERSION,
            found: found.to_string(),
        })
}

/// This crate's own translations: one `.ftl` per language, embedded and registered (§2.1).
pub const TABLE: i18n::Resources = i18n::Resources {
    name: "padparadscha-common",
    files: &[
        ("en", include_str!("../locales/en/padparadscha-common.ftl")),
        ("ja", include_str!("../locales/ja/padparadscha-common.ftl")),
    ],
};

/// The app's foundation. The app supplies bevy's window and render plugins (§2.1).
pub struct CommonPlugin {
    /// Names the directories and the MCP server's own name on the connection.
    pub app_name: &'static str,
    /// This GUI's version, for the handshake Task 9 binds.
    pub version: &'static str,
}

impl Plugin for CommonPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_egui::EguiPlugin::default())
            .insert_resource(LookStore::default(self.app_name))
            // The primary egui context is not available at Startup. Initialize once in its
            // first pass, before installing the language for that frame (spec §2.1, §3.2).
            .add_systems(
                EguiPrimaryContextPass,
                (
                    apply_look.run_if(bevy::ecs::schedule::common_conditions::run_once),
                    setup_i18n.run_if(bevy::ecs::schedule::common_conditions::run_once),
                    install_i18n,
                )
                    .chain(),
            );
    }
}

/// Device-local look and config directory (§4.1.2). Task 9 extends the session plumbing.
#[derive(Resource)]
pub struct LookStore {
    pub dir: PathBuf,
    pub look: Look,
    app_name: &'static str,
}

impl LookStore {
    fn default(app_name: &'static str) -> Self {
        Self {
            dir: PathBuf::new(),
            look: Look::default(),
            app_name,
        }
    }
}

fn self_app(store: &LookStore) -> &'static str {
    store.app_name
}

/// Use corundum's directories and its persisted look, not a second settings implementation.
fn apply_look(mut ctx: EguiContexts, mut state: ResMut<LookStore>, mut commands: Commands) {
    let dirs = corundum_framework::platform::dirs::AppDirs::new(self_app(&state))
        .expect("the platform has app directories");
    state.dir = dirs.config().to_path_buf();
    commands.insert_resource(Settings::load(&state.dir));
    let ctx = ctx
        .ctx_mut()
        .expect("the primary egui context exists in its pass");
    store::attach(ctx, store::Store::at(state.dir.join(store::FILE_NAME)));
    state.look = look::current(ctx).unwrap_or_default();
}

/// This crate's messages, added after corundum's own.
fn setup_i18n(mut ctx: EguiContexts) {
    i18n::register(
        ctx.ctx_mut()
            .expect("the primary egui context exists in its pass"),
        TABLE,
    );
}

/// Apply the viewer's language to the registered resources each frame (§3.2 ②).
fn install_i18n(mut ctx: EguiContexts, state: Res<LookStore>) {
    i18n::install(
        ctx.ctx_mut()
            .expect("the primary egui context exists in its pass"),
        &state.look.language,
    );
}
