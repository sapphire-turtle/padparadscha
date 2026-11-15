//! The tabletop engine, as one bevy plugin (spec §2.1). What the four clients add on top of their own
//! window and render plugins.

mod session;
mod table;

pub use session::{Body, Session};
pub use table::{SpikeCoin, SpikeTable, Table, TablePlugin};
