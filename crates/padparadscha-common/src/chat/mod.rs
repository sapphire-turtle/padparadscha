//! The chat panel: egui widgets inside the bevy window, not a separate shell.
mod bridge;
mod panel;
mod state;

pub use bridge::{Join, line_of, render, to_user_message};
pub use panel::chat_panel;
pub use state::{ChatEntry, ChatRole, ChatState};
