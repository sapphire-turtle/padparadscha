//! The tabletop engine, as one bevy plugin (spec §2.1): the client adds its own window and render
//! plugins, then this one — the crate never owns a window, and no eframe is anywhere (spec §13.2). This task's
//! version is minimal on purpose: two Resources, two mesh handles. Task 6 adds `Table` and `Session`
//! alongside them, Task 7 `physics`, Task 8 `toss`; no one of them remakes this file.

use bevy::prelude::*;

/// The table, in the shape §5.1's `Table` has: its dimensions and its wall. Bodies are only what is
/// placed on top of it. This spike's placeholder until Task 6's `Table` — with `normal()`, the read-out's
/// second side (§6.4) — replaces it.
#[derive(Resource, Clone, Copy)]
pub struct SpikeTable {
    pub half_width_m: f64,
    pub half_depth_m: f64,
    pub wall_m: f64,
}

/// §6.1's coin, in SI metres as f64 (the sim side keeps no fixed point, §5.2).
#[derive(Resource, Clone, Copy)]
pub struct SpikeCoin {
    pub diameter_m: f64,  // 24.0mm
    pub thickness_m: f64, // 1.8mm
    pub mass_kg: f64,     // 5.0g
}

/// The engine's one plugin (§2.1). No `update_mode`: headless and web have no window to idle.
pub struct TablePlugin;

impl Plugin for TablePlugin {
    fn build(&self, app: &mut App) {
        // The table and the body on it, as bevy Resources — which is what makes them resources. The two
        // spike tests overwrite these defaults with the case's own values, and read them back.
        app.init_resource::<Assets<Mesh>>()
            .insert_resource(SpikeTable {
                half_width_m: 0.9,
                half_depth_m: 0.6,
                wall_m: 0.02,
            })
            .insert_resource(SpikeCoin {
                diameter_m: 0.024,
                thickness_m: 0.001_8,
                mass_kg: 0.005,
            });
        // The surface and the coin, as mesh handles on the store: §13.3's table with a body on it. Placeholder
        // shapes, at §6.1's real sizes — Task 6 builds the surface from `Table`, Task 7 extrudes the disc.
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Rectangle::default());
        meshes.add(Circle::new(0.012));
    }
}
