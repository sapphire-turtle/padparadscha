//! §13.2's gate: does the whole stack — bevy with no default features + bevy_egui + corundum-framework +
//! rapier3d — close for `wasm32-unknown-unknown`, and is a table with bodies on it the thing that
//! comes out of it (§13.3)? Host runs on every OS of the matrix; the browser run rides the `wasm` job.

use bevy::prelude::*;
use padparadscha_table::{SpikeCoin, SpikeTable, TablePlugin};
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

/// The table's own dimensions are not what this asks; that it is a `Resource` bevy can hold, with
/// the body's own dimensions of §6.1 on it.
#[test]
fn a_table_with_a_body_constructs_on_this_target() {
    let mut app = App::new();
    app.insert_resource(SpikeTable {
        half_width_m: 0.45,
        half_depth_m: 0.30,
        wall_m: 0.02,
    });
    app.insert_resource(SpikeCoin {
        diameter_m: 0.024,
        thickness_m: 0.001_8,
        mass_kg: 0.005,
    });
    app.add_systems(Startup, |res: Res<SpikeCoin>| {
        assert!(res.diameter_m > res.thickness_m, "a disc, not a sphere");
    });
    app.finish();
    app.update();
    app.cleanup();
}

// The browser run, on the web backend + WebGL2 (§13.2): the surface and the coin exist there as mesh
// handles with an id() each. Only a run says that; a cargo check only says the crate builds.
#[test]
#[wasm_bindgen_test]
fn the_table_and_the_coin_are_mesh_handles_in_a_browser() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, TablePlugin));
    app.finish();
    app.update();
    let meshes = app.world().resource::<Assets<Mesh>>();
    assert_eq!(
        meshes.iter().count(),
        2,
        "the table's surface and the coin, as mesh handles"
    );
}
