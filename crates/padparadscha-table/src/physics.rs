//! Rigid bodies and physics (spec §5.2, §6.1-6.3). The physics types are rapier3d's own
//! f64 vectors; the only fixed point is the read-out (Task 5), so nothing here is fixed.

use rapier3d::prelude::*;

use crate::{session::Body, table::Table};

/// §6.1's coin, in SI units. One body shape, not a per-game rebuild: coin toss and the dice
/// that come later are the same "body bouncing on a table".
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Coin {
    pub diameter_m: f64,
    pub thickness_m: f64,
    pub mass_kg: f64,
}

impl Coin {
    /// §6.1's dimensions, one coin.
    pub const COIN: Self = Self {
        diameter_m: 0.024,
        thickness_m: 0.001_8,
        mass_kg: 0.005,
    };

    /// The body's own face normals, in its own frame: heads and tails (§6.1).
    pub const FACE_A: [f64; 3] = [0.0, 0.0, 1.0];
    pub const FACE_B: [f64; 3] = [0.0, 0.0, -1.0];

    fn radius_m(&self) -> f64 {
        self.diameter_m / 2.0
    }
}

/// The state the seed decides a body is spawned with (spec §6.2): initial position, orientation
/// (axis × angle), velocity and angular velocity.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Launch {
    pub position: [f64; 3],
    pub axis: [f64; 3],
    pub angle: f64,
    pub linvel: [f64; 3],
    pub angvel: [f64; 3],
}

/// The keyframe (spec §7.2): position and orientation, struck when the body came to rest. It is the
/// authoritative outcome reference, and it is used **only as the fallback** for when a peer's own
/// queue has not caught up — not instead of the simulation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Keyframe {
    pub position: [f64; 3],
    /// The resting body's face normal, in world space: §6.4's angle question reads the angle between
    /// this and the table's normal.
    pub face_normal: [f64; 3],
}

/// §6.3's fixed step, in seconds: rapier3d runs at 120Hz.
const STEP_DT: f64 = 1.0 / 120.0;

/// The engine's `simulate` alone (§5.2), on the body's own shape: the table as a fixed body with
/// its wall, the body as a dynamic disc, and the step until the one condition for rest holds.
///
/// Rapier 0.36's `insert_body` inserts no collider: the surface and walls use `insert` with
/// cuboid colliders. Its cylinder's axis is +y, so its collider is turned to the coin's +z face
/// axis before the body's launch rotation. Rest is checked on the inserted body's handle via
/// `world.bodies.get`, not on the fixed table bodies.
pub fn simulate(table: &Table, body: Body, launch: Launch) -> Keyframe {
    let Coin {
        thickness_m,
        mass_kg,
        ..
    } = Coin::COIN;
    let mut world = PhysicsWorld::new();
    world.integration_parameters.dt = STEP_DT;
    // The table's top is at y=0. Four walls bound its surface.
    world.insert(
        RigidBodyBuilder::fixed().translation(vector(0.0, -table.wall_m / 2.0, 0.0)),
        ColliderBuilder::cuboid(table.half_width_m, table.wall_m / 2.0, table.half_depth_m),
    );
    for x in [-table.half_width_m, table.half_width_m] {
        world.insert(
            RigidBodyBuilder::fixed().translation(vector(x, table.wall_m / 2.0, 0.0)),
            ColliderBuilder::cuboid(table.wall_m / 2.0, table.wall_m / 2.0, table.half_depth_m),
        );
    }
    for z in [-table.half_depth_m, table.half_depth_m] {
        world.insert(
            RigidBodyBuilder::fixed().translation(vector(0.0, table.wall_m / 2.0, z)),
            ColliderBuilder::cuboid(table.half_width_m, table.wall_m / 2.0, table.wall_m / 2.0),
        );
    }
    let (body, collider) = match body {
        Body::Coin => (
            RigidBodyBuilder::dynamic()
                .translation(vector(
                    launch.position[0],
                    launch.position[1],
                    launch.position[2],
                ))
                .rotation(vector(
                    launch.axis[0] * launch.angle,
                    launch.axis[1] * launch.angle,
                    launch.axis[2] * launch.angle,
                ))
                .linvel(vector(launch.linvel[0], launch.linvel[1], launch.linvel[2]))
                .angvel(vector(launch.angvel[0], launch.angvel[1], launch.angvel[2])),
            ColliderBuilder::cylinder(thickness_m / 2.0, Coin::COIN.radius_m())
                .rotation(vector(core::f64::consts::FRAC_PI_2, 0.0, 0.0))
                .mass(mass_kg),
        ),
    };
    let (handle, _collider) = world.insert(body, collider);
    // The one condition: the body came to rest (spec 7.2).
    while !world
        .bodies
        .get(handle)
        .expect("the body is there; it was just inserted")
        .is_sleeping()
    {
        world.step();
    }
    let body = world
        .bodies
        .get(handle)
        .expect("the body is there; it was just inserted");
    let q = *body.rotation();
    let n = q * Vector::from(Coin::FACE_A);
    Keyframe {
        position: array(body.translation()),
        face_normal: array(n),
    }
}

/// f64 → rapier3d's own vector, in one place, so the arrays never drift apart.
fn vector(x: f64, y: f64, z: f64) -> Vector {
    Vector::new(x, y, z)
}

fn array(v: Vector) -> [f64; 3] {
    [v.x, v.y, v.z]
}
