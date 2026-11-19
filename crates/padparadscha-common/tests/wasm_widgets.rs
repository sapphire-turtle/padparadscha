//! The other gate: on `wasm32-unknown-unknown`, in a browser run, the widget layer
//! lays out and paints one row. A cargo check says a crate builds; only a run says it works.

use bevy::prelude::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[test]
fn the_widget_layer_paints_one_row_on_this_target() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin::default(),
        bevy::asset::AssetPlugin::default(),
    ));
    app.init_asset::<Shader>();
    app.init_asset::<Image>();
    app.add_plugins(bevy_egui::EguiPlugin::default());
    app.add_systems(PreStartup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });
    let painted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let p = painted.clone();
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        move |mut cx: bevy_egui::EguiContexts| {
            use bevy_egui::egui;
            egui::Area::new(egui::Id::unique("spike-widget")).show(cx.ctx_mut().unwrap(), |ui| {
                ui.label("coin toss"); // the fork re-exports egui (§2.2); no own egui dep
            });
            p.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
    );
    app.finish();
    app.update();
    assert_eq!(
        painted.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "a widget was painted, not only built"
    );
    app.cleanup();
}

// The same, on this target, through a browser: one row laid out and painted on the webgl2 side.
#[test]
#[wasm_bindgen_test]
fn one_widget_row_is_laid_out_and_painted_in_a_browser() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::input::InputPlugin,
        bevy::window::WindowPlugin::default(),
        bevy::asset::AssetPlugin::default(),
    ));
    use bevy_egui::egui;
    app.init_asset::<Shader>();
    app.init_asset::<Image>();
    app.add_plugins(bevy_egui::EguiPlugin::default());
    app.add_systems(PreStartup, |mut commands: Commands| {
        commands.spawn(Camera2d);
    });
    let painted = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let p = painted.clone();
    app.add_systems(
        bevy_egui::EguiPrimaryContextPass,
        move |mut cx: bevy_egui::EguiContexts| {
            egui::Area::new(egui::Id::unique("spike-widget-browser")).show(
                cx.ctx_mut().unwrap(),
                |ui| {
                    ui.label("coin toss");
                },
            );
            p.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        },
    );
    app.finish();
    app.update();
    assert_eq!(
        painted.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "a widget was painted, not only built"
    );
}
