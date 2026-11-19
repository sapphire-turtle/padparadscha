//! The chat panel: egui widgets inside the bevy window, not a separate shell.
mod bridge;
mod panel;
mod state;

pub use bridge::render as bridge_render;
pub use bridge::{Join, line_of, to_user_message};
#[cfg(not(target_arch = "wasm32"))]
mod systems;
pub use panel::chat_panel;
pub use state::{ChatEntry, ChatRole, ChatState};
#[cfg(not(target_arch = "wasm32"))]
pub use systems::{Client, render, send, toss_button};
