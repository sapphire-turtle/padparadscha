//! The app's foundation (spec §4): the look and the settings, the seed and the stream, the ACP session and the chat panel — each added by its own task, and nothing here is a binary.

pub mod acp;
mod app;
pub mod chat;
mod conversation;
mod seed;
mod settings;

pub use app::TABLE as TRANSLATIONS;
pub use app::{API_VERSION, CommonPlugin, LookStore, VersionMismatch, check_api_version};
pub use chat::bridge_render;
pub use chat::bridge_render as render;
pub use chat::{ChatEntry, ChatRole, ChatState, Join, chat_panel, line_of, to_user_message};
pub use conversation::{Actor, Command, Event, Log, Queue};
pub use seed::{Seed, Seq, command_rng, command_stream, face_dot, fixed_range, fixed_unit};
pub use settings::{ServerMode, Settings};
