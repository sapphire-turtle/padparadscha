//! The conversation and the game line, in one crate (spec §5.4): `Seq`, `Actor`, `Command`, `Queue`,
//! `Event`, `Log`. The tabletop engine and the voxel engine both consume these; neither re-declares
//! them. `kind` is a Fluent message id, never a file name (§3.1). `Seed` and `Seq` are the module
//! Task 5's other file of this task's is seed.rs's; this one imports them, never re-declares them.

use crate::seed::Seq;
use serde::{Deserialize, Serialize};

/// Who issued a command: the user, from a client's button, or the agent, from `acp__table__toss_coin`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Actor {
    User,
    Agent,
}

/// What a peer issues. Once accepted it becomes a `Seq`-tagged row (§1's terms).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Command {
    /// who tossed it (§7.2).
    Toss { who: Actor },
    /// A note the user typed — the name of a body, and the reason. Physics does not move (§7.2).
    Log { note: String },
}

/// The plain FIFO queue (this milestone; strict alternating turns are a later, game-side concern). The next
/// `seq` is the queue's own, so a command and the `Event` it produces share exactly one (§5.1).
#[derive(Clone, Default, Debug)]
pub struct Queue {
    pub lines: Vec<(Seq, Command)>,
    /// The seq the next command accepted here is tagged with. Monotonic and the queue's OWN, so a
    /// command and the `Event` it produces share exactly one even after earlier rows drained (5.1) —
    /// never `lines.len()`, which a drained queue leaves at zero.
    pub next: Seq,
}

impl Queue {
    /// A queue resuming alongside a log that already carries rows: the next command continues that log.
    pub fn continuing_from(next: Seq) -> Self {
        Self {
            lines: Vec::new(),
            next,
        }
    }

    /// Accept a command at the end of the queue and hand back the `seq` it is tagged with. A command
    /// that arrives while a simulation is still running waits its turn (§10).
    pub fn push(&mut self, command: Command) -> Seq {
        let seq = self.next;
        self.next = Seq(seq.0 + 1);
        self.lines.push((seq, command));
        seq
    }

    /// Accepted commands, in the order they arrived.
    pub fn take_all(&mut self) -> std::vec::Drain<'_, (Seq, Command)> {
        self.lines.drain(..)
    }
}

/// The state-change row a command produces. `kind` is a Fluent message id: the session (the agent) side
/// renders it with the session's language setting, the client renders it in the viewer's own language
/// (`tr_args`), and switching language later changes nothing already in the log (§3.1).
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Event {
    pub seq: Seq,
    pub kind: &'static str,
    /// Structured as-is: `who`, and what happened. Turning it into text is the reader's job (§3.2).
    pub args: serde_json::Value,
}

impl Event {
    pub fn of(seq: Seq, kind: &'static str, args: serde_json::Value) -> Self {
        Self { seq, kind, args }
    }

    /// §6.6's one event: `coin-toss.outcome`, `args = { who, outcome, attempt }`.
    pub fn coin_toss(seq: Seq, who: Actor, outcome: &str, attempt: u8) -> Self {
        Self::of(
            seq,
            "coin-toss.outcome",
            serde_json::json!({ "who": who, "outcome": outcome, "attempt": attempt }),
        )
    }
}

/// The one append-only sequence commands and `Event`s accumulate under (§1's terms).
#[derive(Clone, Default, Debug, bevy::prelude::Resource)]
pub struct Log(pub Vec<Event>);

impl Log {
    /// Append-only, and the row carries its own `seq`: nothing here rewrites what is already there.
    pub fn append(&mut self, event: Event) {
        debug_assert!(
            self.0.last().is_none_or(|last| event.seq > last.seq),
            "append-only"
        );
        self.0.push(event);
    }

    /// The rows from `from` (inclusive) on — the feed's continuation (§7.3).
    pub fn since(&self, from: Seq) -> Vec<Event> {
        self.0
            .iter()
            .skip_while(|row| row.seq < from)
            .cloned()
            .collect()
    }

    /// The snapshot a client arriving is given: the rows up to `up_to` (inclusive, §7.3).
    pub fn until(&self, up_to: Seq) -> Vec<Event> {
        self.0
            .iter()
            .take_while(|row| row.seq <= up_to)
            .cloned()
            .collect()
    }
}
