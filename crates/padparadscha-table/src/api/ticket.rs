//! The one-time ticket (spec §10): single use, 30 seconds by default, bound to a session id.
//! Unknown, expired or a second use is a 401 at the `GET /ws` handshake; reconnecting is done
//! by re-issuing with a NEW ticket, never by re-presenting the old one.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// How long an issued ticket stays redeemable (§10's default).
pub const DEFAULT_TTL: Duration = Duration::from_secs(30);

/// ticket → (session id, issued at). A ticket is removed the moment it is redeemed, which is
/// what makes the second use fail: the map itself is the single-use record.
#[derive(Default)]
pub struct Tickets(HashMap<String, (String, Instant)>);

impl Tickets {
    /// Issue a fresh ticket bound to `session_id`. Reconnecting calls this again and gets a NEW
    /// ticket (§10) — old ones are not revived.
    pub fn issue(&mut self, session_id: &str, now: Instant) -> String {
        let ticket = uuid::Uuid::new_v4().to_string();
        self.0.insert(ticket.clone(), (session_id.to_string(), now));
        ticket
    }

    /// True exactly once per ticket, within the TTL, for the session it was bound to. The ticket
    /// is consumed either way, so a wrong session or an expired ticket cannot be retried either.
    pub fn redeem(&mut self, session_id: &str, ticket: &str, now: Instant) -> bool {
        match self.0.remove(ticket) {
            Some((bound, issued)) => {
                bound == session_id && now.duration_since(issued) <= DEFAULT_TTL
            }
            None => false,
        }
    }
}
