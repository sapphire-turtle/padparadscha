//! The tabletop engine, as one bevy plugin (spec §2.1). What the four clients add on top of their own
//! window and render plugins.

mod physics;
mod session;
mod table;
mod toss;

pub use physics::{Coin, Keyframe, Launch, simulate};
pub use session::{Body, Session};
pub use table::{SpikeCoin, SpikeTable, Table, TablePlugin};
pub use toss::{CoinOutcome, CoinToss, Outcome};
