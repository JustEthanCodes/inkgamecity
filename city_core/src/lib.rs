//! city_core — foundational, engine-free types for City Floorlines.
//!
//! Everything in this crate compiles and tests with zero Bevy dependencies so it can be
//! linked by the headless server, tools, and asset builders.

#![forbid(unsafe_code)]

pub mod enums;
pub mod grid;
pub mod id;
pub mod intent;
pub mod math;
pub mod spatial;
pub mod time;

pub use enums::*;
pub use grid::ChunkedGrid;
pub use id::*;
pub use intent::{IntentQueue, IntentReject, PlayerIntent};
pub use math::{Aabb3, Obb2, smoothstep};
pub use spatial::{AgentLayer, GridHandle, SpatialGrid};
pub use time::{SIM_HZ, SimSpeed, Tick};
