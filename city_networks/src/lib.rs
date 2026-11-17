//! city_networks — road/rail/utility network primitives and pathfinding (§4.1, §2.6.4).

#![forbid(unsafe_code)]

pub mod bezier;
pub mod graph;
pub mod pathfind;

pub use bezier::{ArcLengthTable, CubicBezier};
pub use graph::{GraphBuilder, LaneNodeKind, NetworkGraph};
pub use pathfind::astar;
