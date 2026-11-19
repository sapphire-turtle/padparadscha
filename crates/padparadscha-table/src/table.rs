//! The tabletop engine, as one bevy plugin (spec §2.1): the client adds its own window and render
//! plugins, then this one — the crate never owns a window, and no eframe is anywhere (spec §13.2). This task's
//! spike version uses two Resources and two mesh handles. `Table` lives alongside them; Task 7 adds
//! `physics`, Task 8 `toss`.

use bevy::prelude::*;

/// The table, in the shape §5.1's `Table` has: its dimensions and its wall. Bodies are only what is
/// placed on top of it. The spike resource remains for the spike tests; `Table` below is the engine's own type, with
/// `normal()` as the read-out's second side (§6.4).
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
        // shapes, at §6.1's real sizes — the physics implementation arrives in Task 7.
        let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
        meshes.add(Rectangle::default());
        meshes.add(Circle::new(0.012));
    }
}

/// One table. The normal is what §6.4's angle is taken against.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Table {
    /// Half of the playing surface's width, in metres.
    pub half_width_m: f64,
    /// Half of its depth, in metres.
    pub half_depth_m: f64,
    /// The wall's height: what a body that reaches it stops against.
    pub wall_m: f64,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            half_width_m: 0.9,
            half_depth_m: 0.6,
            wall_m: 0.02,
        }
    }
}

impl Table {
    /// The surface's normal, in the `f64` the simulation runs in (§5.2: rapier3d's own types;
    /// no fixed point is on the sim side).
    pub fn normal(&self) -> [f64; 3] {
        [0.0, 1.0, 0.0]
    }
}
