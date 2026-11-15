//! §6.1, §6.2, §6.3 and §5.2: the disc with thickness, the body spawned with the state the
//! seed decides, a fixed 120Hz step, and the keyframe struck when it came to rest.

use padparadscha_table::{Body, Coin, Keyframe, Launch, Table, simulate};

#[test]
fn the_coin_is_a_disc_with_thickness_two_faces_and_an_edge() {
    // §6.1's dimensions, SI f64 values.
    assert_eq!(Coin::COIN.diameter_m, 0.024);
    assert_eq!(Coin::COIN.thickness_m, 0.001_8);
    assert_eq!(Coin::COIN.mass_kg, 0.005);
}

#[test]
fn the_engine_carries_simulate_alone_and_rest_is_position_and_orientation() {
    // The one condition for calling rest: "the body came to rest" (spec 7.2). Every peer runs
    // the same thing on its own side from the same command log and seed (spec 5.2).
    let flat = Launch {
        position: [0.0, 0.05, 0.0],
        axis: [1.0, 0.0, 0.0],
        angle: -core::f64::consts::FRAC_PI_2,
        linvel: [0.0, -0.02, 0.0],
        angvel: [0.0, 0.0, 0.0],
    };
    let Keyframe {
        position,
        face_normal,
    } = simulate(&Table::default(), Body::Coin, flat);
    assert!(
        face_normal[1].abs() > 0.999,
        "flat on a face: its normal is the table's own, to the sign"
    );
    assert!(position[1] > 0.0, "on top of the table");
}

#[test]
fn launch_orientation_is_read_out_after_rest_and_repeated_runs_agree() {
    let face_up = Launch {
        position: [0.0, 0.05, 0.0],
        axis: [1.0, 0.0, 0.0],
        angle: -core::f64::consts::FRAC_PI_2,
        linvel: [0.0, -0.02, 0.0],
        angvel: [0.0, 0.0, 0.0],
    };
    let face_down = Launch {
        angle: core::f64::consts::FRAC_PI_2,
        ..face_up
    };
    let table = Table::default();
    let up = simulate(&table, Body::Coin, face_up);
    let down = simulate(&table, Body::Coin, face_down);
    assert!(up.face_normal[1] > 0.999, "{up:?}");
    assert!(down.face_normal[1] < -0.999, "{down:?}");
    assert_eq!(up, simulate(&table, Body::Coin, face_up));
    assert_eq!(down, simulate(&table, Body::Coin, face_down));
    assert!(
        up.position[1] < 0.005,
        "rest on surface, not launch: {up:?}"
    );
}

#[test]
fn surface_and_walls_bound_a_moving_coin() {
    let table = Table {
        half_width_m: 0.08,
        half_depth_m: 0.08,
        wall_m: 0.02,
    };
    for linvel in [
        [0.5, 0.0, 0.0],
        [-0.5, 0.0, 0.0],
        [0.0, 0.0, 0.5],
        [0.0, 0.0, -0.5],
    ] {
        let launch = Launch {
            position: [0.0, 0.01, 0.0],
            axis: [1.0, 0.0, 0.0],
            angle: -core::f64::consts::FRAC_PI_2,
            linvel,
            angvel: [0.0, 0.0, 0.0],
        };
        let rest = simulate(&table, Body::Coin, launch);
        assert!(
            rest.position[1] > 0.0 && rest.position[1] < 0.025,
            "{linvel:?}: {rest:?}"
        );
        assert!(
            rest.position[0].abs() < table.half_width_m,
            "{linvel:?}: {rest:?}"
        );
        assert!(
            rest.position[2].abs() < table.half_depth_m,
            "{linvel:?}: {rest:?}"
        );
    }
}
