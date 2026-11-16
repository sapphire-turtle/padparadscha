//! The conversation lines displayed on one client.
use crate::{Event, Seq};
use bevy::prelude::Resource;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatRole {
    User,
    Agent,
    System,
}

#[derive(Clone, PartialEq, Debug)]
pub struct ChatEntry {
    pub role: ChatRole,
    pub text: String,
    pub seq: Option<Seq>,
}

#[derive(Resource, Default)]
pub struct ChatState(pub Vec<ChatEntry>);

impl ChatState {
    pub fn push(&mut self, role: ChatRole, text: impl Into<String>) {
        self.0.push(ChatEntry {
            role,
            text: text.into(),
            seq: None,
        });
    }

    pub fn push_line(&mut self, event: &Event, text: impl Into<String>) {
        self.0.push(ChatEntry {
            // Task 13 ruling: the row carries its issuing actor, not a synthetic system role.
            role: match event.args["who"].as_str() {
                Some("User") => ChatRole::User,
                Some("Agent") => ChatRole::Agent,
                _ => ChatRole::System,
            },
            text: text.into(),
            seq: Some(event.seq),
        });
    }

    pub fn next_wanted(&self) -> Option<Seq> {
        self.0
            .iter()
            .filter_map(|e| e.seq)
            .max()
            .map(|s| Seq(s.0 + 1))
    }
}
