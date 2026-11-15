//! The tabletop engine, as one bevy plugin (spec §2.1). What the four clients add on top of their own
//! window and render plugins.

mod table;

pub use table::{SpikeCoin, SpikeTable, TablePlugin};
