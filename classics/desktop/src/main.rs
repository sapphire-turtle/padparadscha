//! The same game server with a Bevy window (spec §8).

#[cfg(not(target_arch = "wasm32"))]
fn main() -> anyhow::Result<()> {
    use bevy::prelude::*;
    use bevy_egui::EguiPrimaryContextPass;
    use padparadscha_classics_core::COIN_TOSS;
    use padparadscha_common::{ChatState, CommonPlugin, Seed, chat};
    use padparadscha_table::{Session, TablePlugin, api};

    // Keep the runtime and server alive for the window's lifetime.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let seed = Seed(getrandom::u64().expect("the OS's own randomness"));
    let server = runtime.block_on(api::serve_sim(
        Session::new(COIN_TOSS.name, seed),
        "127.0.0.1:0".parse()?,
    ))?;
    tracing::info!(addr = %server.addr(), "the game server is listening");
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: COIN_TOSS.name.into(),
            ..default()
        }),
        ..default()
    }));
    app.insert_resource(bevy::winit::WinitSettings::desktop_app());
    app.add_plugins((
        CommonPlugin {
            app_name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
        },
        TablePlugin,
    ));
    app.insert_resource(chat::Client::new(server.addr())?)
        .init_resource::<ChatState>();
    app.add_systems(
        EguiPrimaryContextPass,
        (chat::render, chat::send, chat::toss_button).chain(),
    );
    app.run();
    drop(server);
    drop(runtime);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn main() {}
