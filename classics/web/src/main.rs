//! The wasm client (§2.1, §8): no bundled server, only the same-origin API.

use bevy::prelude::*;
use padparadscha_common::CommonPlugin;
use padparadscha_table::TablePlugin;

pub mod origin;
#[cfg(all(test, target_arch = "wasm32"))]
mod roundtrip;

/// The same common and table plugins as desktop, without DefaultPlugins or a server.
pub fn app() -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin::default(),
        bevy::asset::AssetPlugin::default(),
        bevy::render::RenderPlugin::default(),
    ));
    // CommonPlugin owns EguiPlugin: adding it here too would register a duplicate plugin.
    app.add_plugins((
        CommonPlugin {
            app_name: "padparadscha-classics",
            version: env!("CARGO_PKG_VERSION"),
        },
        TablePlugin,
    ));
    // TablePlugin owns the rendering resources. Table and CoinToss are plain model values,
    // not Resources; the simulation remains on the origin's server.
    app
}

#[cfg(target_arch = "wasm32")]
fn main() {
    origin::set_token_from_query(
        &web_sys::window()
            .expect("a browser")
            .location()
            .search()
            .unwrap_or_default(),
    );
    app().run();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
