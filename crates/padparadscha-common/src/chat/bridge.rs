//! Session-language and viewer-language renderings of structured game events.
use crate::{ChatState, Event, Log, Seed, Seq, Settings};
use bevy_egui::egui;
use corundum_framework::gui::i18n::{self, Arg};

pub fn line_of(ctx: &egui::Context, event: &Event) -> String {
    let args: Vec<(&str, Arg)> = event
        .args
        .as_object()
        .map(|m| m.iter().map(|(k, v)| (k.as_str(), to_arg(v))).collect())
        .unwrap_or_default();
    // Fluent message IDs cannot contain a dot; corundum's lookup uses get_message,
    // not attribute lookup. The canonical event ID stays unchanged in the log.
    let id = if event.kind == "coin-toss.outcome" {
        "coin-toss-outcome"
    } else {
        event.kind
    };
    i18n::tr_args(ctx, id, &args)
}

fn to_arg(v: &serde_json::Value) -> Arg {
    match v.as_str() {
        Some(s) => Arg::Str(s.to_string()),
        None => Arg::Num(v.as_f64().unwrap_or(0.0)),
    }
}

pub fn to_user_message(
    session: &i18n::LanguageSettings,
    event: &Event,
) -> (String, serde_json::Value) {
    let ctx = egui::Context::default();
    i18n::register(&ctx, crate::app::TABLE);
    i18n::install(&ctx, session);
    (line_of(&ctx, event), event.args.clone())
}

pub fn render(ctx: &egui::Context, rows: &[Event], state: &mut ChatState) {
    for row in rows {
        state.push_line(row, line_of(ctx, row));
    }
}

/// A snapshot of one log. Set its id to the bound session's id with `for_session`.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Join {
    pub id: String,
    pub seed: Seed,
    pub rows: Vec<Event>,
}

impl Join {
    /// Compatibility with the plan's settings-only form: no session ID is supplied there.
    #[allow(clippy::self_named_constructors, reason = "plan specifies Join::join")]
    pub fn join(_settings: &Settings, seed: Seed, log: &Log, from_seq: Option<Seq>) -> Self {
        Self::for_session("", seed, log, from_seq)
    }
    /// Construct the response from the actual bound session's id.
    pub fn for_session(
        id: impl Into<String>,
        seed: Seed,
        log: &Log,
        from_seq: Option<Seq>,
    ) -> Self {
        let rows = match from_seq {
            // Task 13: from_seq is the last received row, exclusively.
            Some(from) => log.0.iter().filter(|row| row.seq > from).cloned().collect(),
            None => log
                .0
                .last()
                .map_or_else(Vec::new, |last| log.until(last.seq)),
        };
        Self {
            id: id.into(),
            seed,
            rows,
        }
    }
}
