// §13.1's prototype: does rapier3d give the same resting pose on every target, at
//! the granularity §6.4's angle adjudication needs? §12: check agreement at that
//! granularity, not down to the low bits of f64, and do not treat a low-bit
//! difference that leaves the adjudication unchanged as a mismatch. The world itself is
//! rapier3d's (the f64, SIMD, aarch64 FMA and wasm float paths of §13.1 are what is being
//! compared); the read-out is the fixed point alone.

use fixed::types::I16F16;
use rapier3d::prelude::*;

/// §6.3's fixed step: the world runs at 120Hz, the same step Task 7's `simulate` runs at.
const STEP_DT: f64 = 1.0 / 120.0;

/// One command's seed, printed so the other targets read the same bytes. The real
/// (seed, seq) stream arrives with Task 5; the spike needs one fixed state only.
const SEED: u64 = 0x5EED_5EED_5EED_5EED;

/// The table's normal, in the same f64 the simulation runs in: +y, the surface the coin
/// rests on. The table is one fixed cuboid, so its own normal *is* this.
const TABLE_NORMAL: [f64; 3] = [0.0, 1.0, 0.0];

/// The disc's own axis in its own frame (rapier3d's 3D cylinder's axis is +y): the two
/// faces §6.4's question is about are the ± of this, which one came up is the sign of the
/// dot, and the world decides it.
const FACE_LOCAL: [f64; 3] = [0.0, 1.0, 0.0];

/// §6.1's coin, SI f64: a cylinder of radius 12mm and half-height 0.9mm, not a sphere.
const COIN_RADIUS_M: f64 = 0.012;
const COIN_HALF_HEIGHT_M: f64 = 0.000_9;
const COIN_MASS_KG: f64 = 0.005;

/// The state one case spawns the coin with: position, orientation (axis × angle), and the
/// non-trivial linear and angular velocities of §6.2.
pub struct SpawnPose {
    pub position: [f64; 3],
    pub axis: [f64; 3],
    pub angle: f64,
    pub linvel: [f64; 3],
    pub angvel: [f64; 3],
}

/// The dot product of §6.4, in I16F16 alone: each component is dropped onto the same fixed
/// point (`from_num`) before it is multiplied, so no `f64` arithmetic of a target is left to sway
/// the adjudication (§7.1), and no `cos` and no `atan2` is asked what they think.
fn face_cos(faithful: [f64; 3], table_normal: [f64; 3]) -> I16F16 {
    let drop = |v: f64| I16F16::from_num(v);
    let mut dot = I16F16::from_bits(0);
    for (a, b) in std::iter::zip(faithful, table_normal) {
        dot += drop(a) * drop(b);
    }
    dot
}

/// cos(45°) at I16F16: 0.7071_0678 × 65536 = 46341 raw.
fn cos45() -> I16F16 {
    I16F16::from_bits(46341)
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

fn v(xyz: [f64; 3]) -> Vector {
    Vector::new(xyz[0], xyz[1], xyz[2])
}

/// The world every case runs on: `PhysicsWorld::new()`, whose own top-face normal is
/// `TABLE_NORMAL`, and whose step is §6.3's fixed 120Hz one (dt, never a frame rate, so
/// every target steps the same 1/120 s of physics).
fn world() -> PhysicsWorld {
    let mut world = PhysicsWorld::new();
    world.integration_parameters.dt = STEP_DT;
    world
}

/// The fixed bodies every case shares: the table's surface (one cuboid whose own top-face normal
/// *is* `TABLE_NORMAL`) and its two fixed side walls (§5.1's wall).
fn add_table(world: &mut PhysicsWorld) {
    let (surface, _collider) = world.insert(
        RigidBodyBuilder::fixed().translation(v([0.0, -0.01, 0.0])),
        ColliderBuilder::cuboid(0.45, 0.01, 0.30),
    );
    assert!(
        world
            .bodies
            .get(surface)
            .expect("the surface is there")
            .translation()
            .y
            < 0.0,
        "the surface is under the coin"
    );
    for x in [-0.45, 0.45] {
        let (_wall, _collider) = world.insert(
            RigidBodyBuilder::fixed().translation(v([x, 0.02, 0.0])),
            ColliderBuilder::cuboid(0.01, 0.02, 0.30),
        );
    }
}

/// The coin as one dynamic cylinder body, spawned with the state §6.2 names: position,
/// orientation (the axis × angle, as the angular vector the builder takes), and the non-trivial
/// linear and angular velocities of the case.
fn spawn_coin(world: &mut PhysicsWorld, spawn_pose: &SpawnPose) -> RigidBodyHandle {
    let angular = v([
        spawn_pose.axis[0] * spawn_pose.angle,
        spawn_pose.axis[1] * spawn_pose.angle,
        spawn_pose.axis[2] * spawn_pose.angle,
    ]);
    let body = RigidBodyBuilder::dynamic()
        .translation(v(spawn_pose.position))
        .rotation(angular)
        .linvel(v(spawn_pose.linvel))
        .angvel(v(spawn_pose.angvel));
    let collider = ColliderBuilder::cylinder(COIN_HALF_HEIGHT_M, COIN_RADIUS_M).mass(COIN_MASS_KG);
    let (handle, _collider) = world.insert(body, collider);
    handle
}

/// §7.2's one condition, run for real: `PhysicsWorld::step` at dt = 1.0/120.0 until the body sleeps.
fn step_until_sleeping(world: &mut PhysicsWorld, coin: RigidBodyHandle) {
    let mut frames = 0usize;
    while !world
        .bodies
        .get(coin)
        .expect("the body is there; it was just inserted")
        .is_sleeping()
    {
        world.step();
        frames += 1;
        assert!(
            frames < 100_000,
            "the body must come to rest within 100_000 frames of dt = 1/120"
        );
    }
}

/// One case run for real: the world above, the coin spawned at `spawn_pose` with its
/// non-trivial velocities, stepped at `dt = 1.0/120.0` until it sleeps, and its resting pose
/// read out at the read-out's granularity — each f64 component dropped through
/// `I16F16::from_num` before the dot product, and the whole line printed at 8 decimal
/// places (4 finer than §6.4's 0.4° needs). The `String` is that line, the `u64` its
/// FNV-1a hash: what a run on x86_64, aarch64, windows and wasm prints and compares
/// against (§13.1's verdict).
pub fn resting_pose_case(name: &str, spawn_pose: SpawnPose) -> (String, u64) {
    let mut world = world();
    add_table(&mut world);
    let coin = spawn_coin(&mut world, &spawn_pose);
    step_until_sleeping(&mut world, coin);
    let body = world
        .bodies
        .get(coin)
        .expect("the body is there; it was just inserted");
    let resting = *body.rotation(); // the unit quaternion, the way the read-out is read
    let face_normal = resting * v(FACE_LOCAL);
    let dot = face_cos([face_normal.x, face_normal.y, face_normal.z], TABLE_NORMAL);
    let pose = format!(
        "{name} face=[{:.16},{:.16},{:.16}] cos={:.8} pos=[{:.8},{:.8},{:.8}] seed={:#018x}",
        face_normal.x,
        face_normal.y,
        face_normal.z,
        dot.to_num::<f64>(),
        body.translation().x,
        body.translation().y,
        body.translation().z,
        SEED,
    );
    let hash = fnv1a(&pose);
    (pose, hash)
}

/// The four cases the spike runs. Each one is one command's state of §6.2 (position,
/// orientation as axis × angle, and the velocities); the seed of that stream is the `SEED`
/// above, printed with every line. What each case's own run prints and hashes is what the
/// tests below read back, and what the other targets' runs are compared against.
fn cases() -> Vec<(&'static str, SpawnPose)> {
    vec![
        // The face up, a small tilt off the table's own normal, spinning about the vertical:
        // the +y component of the resting face normal stays over cos(45°), positive.
        (
            "heads",
            SpawnPose {
                position: [0.0, 0.08, 0.0],
                axis: [0.0, 0.0, 1.0],
                angle: 0.25,
                linvel: [0.0, -0.9, 0.0],
                angvel: [0.0, 9.0, 0.0],
            },
        ),
        // The other face up: the same tilt and the same spin, the flipped face.
        (
            "tails",
            SpawnPose {
                position: [0.0, 0.08, 0.0],
                axis: [0.0, 0.0, 1.0],
                angle: 0.25 + core::f64::consts::PI,
                linvel: [0.0, -0.9, 0.0],
                angvel: [0.0, 9.0, 0.0],
            },
        ),
        // The bounce case: the higher drop, the harder velocity, and the spin about the vertical
        // alone — so through the bounces the face stays the one it started on.
        (
            "bouncing-high",
            SpawnPose {
                position: [0.0, 0.24, 0.0],
                axis: [0.0, 1.0, 0.0],
                angle: 0.02,
                linvel: [0.06, -1.6, 0.03],
                angvel: [0.0, 12.0, 0.0],
            },
        ),
        // The boundary case: the spawn sits at the boundary itself — the measured rest
        // (0.73 rad, cos 0.7453 at I16F16) lands a hair past cos(45°), which is exactly
        // the finest question §13.1 asks, and close to the wall it lands by. A boundary probe, not
        // an edge case: whichever side its cos lands on is what its printed line measures.
        (
            "boundary",
            SpawnPose {
                position: [0.42, 0.05, 0.0],
                axis: [0.0, 0.0, 1.0],
                angle: 1.0,
                linvel: [0.0, -0.3, 0.0],
                angvel: [0.2, 0.0, 0.0],
            },
        ),
    ]
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn resting_pose_hash_per_case_is_the_one_the_other_targets_must_reach() {
    for (name, spawn_pose) in cases() {
        let (pose, hash) = resting_pose_case(name, spawn_pose);
        // Printed on this machine. A run on aarch64, windows or wasm prints the same line and
        // the same hash; one that prints a different hash is §13.1's verdict, and the line above
        // is what names which pose disagreed, and at what granularity.
        assert_eq!(hash, fnv1a(&pose), "the pose must be self-consistent");
        assert!(
            pose.contains("seed=0x5eed5eed5eed5eed"),
            "the printed form carries the seed, so another target's run prints it byte for byte"
        );
        println!("{pose} {hash:#018x} {}", std::env::consts::OS);
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn three_named_cases_lands_on_the_side_their_names_say_and_the_fourth_rests_on_the_boundary() {
    // §6.4 read back off the world's own lines: under 45° is that face (the printed cos, over
    // the printed cos(45°), is the same question); 45° or more stands on the edge, cannot be
    // adjudicated, and re-tosses on its own. Which face came up is the sign of the printed cos.
    let c45 = cos45().to_num::<f64>(); // 0.7071075...
    for name in ["heads", "tails", "bouncing-high"] {
        let (pose, _hash) = resting_pose_case(name, case_of(name));
        let cos = cos_of(&pose);
        assert!(
            cos.abs() > c45,
            "{name}: under 45°, one face points up, not the edge"
        );
        // Which of the two faces came up is the sign, and it is the case's own name.
        let up = cos > 0.0;
        assert!(
            up == (name == "heads" || name == "bouncing-high"),
            "{name}: the sign names which face came up"
        );
    }
    // The boundary probe: its cos lands within a hair of cos(45°) — either side is a
    // pass. Which side, at what value, and its hash, are what every target must print the same.
    let (edge, _hash) = resting_pose_case("boundary", case_of("boundary"));
    let edge_cos = cos_of(&edge);
    assert!(
        (edge_cos - c45).abs() < 0.05,
        "boundary: a hair either side of cos(45°), on the finest question there is"
    );
}

/// The one cos printed on a pose line — read back as a string, so this test never re-runs the
/// arithmetic of one target in place of the arithmetic of that same target.
fn cos_of(pose: &str) -> f64 {
    let rest = pose
        .split(" cos=")
        .nth(1)
        .expect("the line carries the cos");
    rest.split(' ')
        .next()
        .expect("the cos ends at the next space")
        .parse::<f64>()
        .expect("the printed cos parses")
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn under_45_is_that_face_and_45_or_more_is_not_adjudicable() {
    // §6.4: under 45° is that face (cos over cos(45°) is the same question); 45° or
    // more stands on the edge, cannot be adjudicated, and re-tosses on its own. The world above
    // is what produces the real normals below.
    assert!(
        face_cos(FACE_LOCAL, TABLE_NORMAL) > cos45(),
        "under 45°: that face"
    );
    assert!(
        face_cos([1.0, 0.0, 0.0], TABLE_NORMAL) < cos45(),
        "on the edge: no adjudication here"
    );
}

fn case_of(wanted: &str) -> SpawnPose {
    cases()
        .into_iter()
        .find(|(name, _)| *name == wanted)
        .expect("the case is there")
        .1
}
