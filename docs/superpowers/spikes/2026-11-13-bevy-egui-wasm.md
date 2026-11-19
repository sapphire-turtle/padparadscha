# bevy + bevy_egui + rapier3d on wasm32 — build does **not** close

Date measured: 2026-11-13 (plan date)
Toolchain actually used: `rustc 1.99.0 (b940084d7 2026-09-28)`; host `x86_64-unknown-linux-gnu`.
The `wasm32-unknown-unknown` standard library was installed successfully.

## Verdict (spec §13.2, §13.3)

**Original spike verdict: the wasm build did not close.** `corundum-framework-gui 0.11.0` failed to compile on `wasm32-unknown-unknown`. Consequently the combined dependency stack did not build and neither browser test ran. The later build resolution is recorded below; browser results remain unmeasured.

This is not a missing-runner-only limitation and it is not something to defer to Task 3 as though compilation had passed. No substitute framework, feature set, dependency patch, or alternative architecture was improvised.

## Pin form

The root `[workspace.dependencies]` pins corundum-framework to the fork git tag `v0.11.0-wasm` with default features disabled; members use `corundum-framework.workspace = true`.

## Four requested crate/target build results

| Crate and target | Actual result | Evidence |
| --- | --- | --- |
| `padparadscha-common`, `x86_64-unknown-linux-gnu` | **PASS** | `cargo test -p padparadscha-common` exited 0; unit test and both widget spike functions passed. This is a host API/UI-pass run, not a browser or GPU-render result. |
| `padparadscha-table`, `x86_64-unknown-linux-gnu` | **PASS** | `cargo test -p padparadscha-table` exited 0; Task 1's three determinism tests and both table spike functions passed. |
| `padparadscha-common`, `wasm32-unknown-unknown` | **FAIL** | The required two-package check exited 101 in `corundum-framework-gui`; common therefore does not close. |
| `padparadscha-table`, `wasm32-unknown-unknown` | **BLOCKED, NOT CLOSED** | Table depends on common and the same stack; the required two-package check failed before a table artifact could be checked. No independent successful table-wasm result is claimed. |

`cargo check --workspace` separately passed on the host with zero warnings.

## Exact wasm failure

Command:

```text
cargo check -p padparadscha-common -p padparadscha-table --target wasm32-unknown-unknown
```

Exit status: **101**. Relevant complete error set from the final run:

```text
    Checking corundum-framework-gui v0.11.0
error[E0433]: cannot find `FileDialog` in `rfd`
   --> corundum-framework-gui-0.11.0/src/platform/mod.rs:71:14
    |
 71 |         rfd::FileDialog::new().set_title(title).pick_folder()
    |              ^^^^^^^^^^ could not find `FileDialog` in `rfd`

error[E0433]: cannot find `FileDialog` in `rfd`
   --> corundum-framework-gui-0.11.0/src/platform/mod.rs:151:31
    |
151 |         let mut dialog = rfd::FileDialog::new();
    |                               ^^^^^^^^^^ could not find `FileDialog` in `rfd`

error[E0425]: cannot find type `FileDialog` in crate `rfd`
   --> corundum-framework-gui-0.11.0/src/platform/mod.rs:150:29
    |
150 |     fn dialog(self) -> rfd::FileDialog {
    |                             ^^^^^^^^^^ not found in `rfd`

error[E0425]: cannot find value `program` in this scope
   --> corundum-framework-gui-0.11.0/src/platform/mod.rs:367:28
    |
367 |     if let Some(program) = program {
    |                            ^^^^^^^ not found in this scope

error: could not compile `corundum-framework-gui` (lib) due to 4 previous errors
warning: build failed, waiting for other jobs to finish...
```

Each `FileDialog` diagnostic points to `rfd 0.17.2/src/lib.rs:139`, where `FileDialog` is gated by `#[cfg(not(target_arch = "wasm32"))]`. The fourth error is independent of those dialog types: wasm32 defines none of the target-specific `program` bindings in `open_in_file_manager` before the unconditional use.

### Resolution (Task 3)

The fork branch `wasm`, tag `v0.11.0-wasm` (commit `81a8dfc9fb86350ae6e9d755da1e41c06aa42ca0`), closes the original four platform leaks: `pick_folder`, `FilePicker::pick_file` / `pick_files`, the `dialog` constructor, and `open_in_file_manager` now gate wasm to their no-dialog / no-program arms. `DESKTOP` also excludes wasm. Padparadscha is re-pinned to this tag. The whole-workspace wasm build now closes:

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo check --workspace --target wasm32-unknown-unknown
Finished `dev` profile [unoptimized + debuginfo] target(s)
exit 0
```

The command used the local `file://` git override at the same tag; the committed pin uses HTTPS and requires the tag to be pushed. An intermediate whole-workspace run after the fork fix exited 101:

```text
cargo check --workspace --target wasm32-unknown-unknown
error: Only features sync,macros,io-util,rt,time are supported on wasm.
 --> tokio-1.54.1/src/lib.rs:516:1
516 | compile_error!("Only features sync,macros,io-util,rt,time are supported on wasm.");
exit 101
```

The root tokio entry now carries only the wasm-safe `macros`, `sync`, and `time` core; `rt-multi-thread` rides on the host-only classics binary dependencies. Browser tests have not been run.

## Widget and mesh-handle runs

Host, after adapting the spike to bevy_egui 0.43.1 / egui 0.37.0 APIs:

```text
padparadscha-common:
  tests::it_works ... ok
  one_widget_row_is_laid_out_and_painted_in_a_browser ... ok (host harness)
  the_widget_layer_paints_one_row_on_this_target ... ok

padparadscha-table:
  under_45_is_that_face_and_45_or_more_is_not_adjudicable ... ok
  resting_pose_hash_per_case_is_the_one_the_other_targets_must_reach ... ok
  three_named_cases_lands_on_the_side_their_names_say_and_the_fourth_rests_on_the_boundary ... ok
  a_table_with_a_body_constructs_on_this_target ... ok
  the_table_and_the_coin_are_mesh_handles_in_a_browser ... ok (host harness)
```

The names ending `in_a_browser` above are shared function names, **not evidence of a browser run**. The explicit `#[test]` alongside each `#[wasm_bindgen_test]` makes wasm-bindgen-test 0.3.80's non-wasm function body execute in the ordinary host harness as well. Host table mesh assertions found two mesh handles; host widget assertions ran a labelled egui area through `EguiPrimaryContextPass`.

At the original spike, **no wasm widget-paint result, mesh-handle result, or browser result existed** because the dependency check failed first. The build is now fixed as recorded above; browser execution has not been repeated.

Browser tooling was nevertheless checked locally. Firefox 159.0 is installed. `wasm-bindgen`, `wasm-bindgen-test-runner`, `geckodriver`, `chromedriver`, and Chrome/Chromium executables were not found; `CHROMEDRIVER`, `GECKODRIVER`, and `WASM_BINDGEN_TEST_RUNNER` are unset. Browser execution is therefore unavailable locally for this independent reason too.

## eframe and resolved egui

The plan's `cargo tree ... -e` form needs an edge value on this Cargo. Using the equivalent valid `--edges normal` form:

```text
cargo tree -p padparadscha-common --edges normal --target wasm32-unknown-unknown | grep -c eframe
0
```

This zero-eframe graph result alone did not establish compilation; the later workspace check above does.

```text
cargo tree -p padparadscha-common -e normal -i egui
```

resolves one egui package version, **egui 0.37.0**, through bevy_egui 0.43.1 (fork revision `9fcb2456`), corundum-framework-gui 0.11.0, and egui_extras 0.37.0.
