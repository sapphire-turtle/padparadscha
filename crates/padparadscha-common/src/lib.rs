//! The app's foundation (spec §4): the look, settings and API-version handshake.
//! Task 9 adds the ACP session and chat panel; this crate is never a window or binary.

mod app;
mod settings;

pub use app::{API_VERSION, CommonPlugin, LookStore, VersionMismatch, check_api_version};
pub use settings::{ServerMode, Settings};
