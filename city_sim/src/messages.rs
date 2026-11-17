//! Buffered messages (Bevy 0.17+ API: `Message` + `add_message`). Observer-style one-shot
//! events should derive `Event` instead; these are drained once per tick.

use bevy_ecs::message::Message;
use city_core::{LaneNodeId, PathId, PlayerId, TravelMode};

#[derive(Message, Clone, Debug)]
pub struct IntentRejected {
    pub player: PlayerId,
    pub reason: city_core::IntentReject,
}

#[derive(Message, Clone, Debug)]
pub struct PathRequest {
    pub origin: LaneNodeId,
    pub destination: LaneNodeId,
    pub mode: TravelMode,
    pub owner: u64,
}

#[derive(Message, Clone, Debug)]
pub struct PathResultReady {
    pub job_seq: u64,
    pub owner: u64,
    pub path: Option<PathId>,
}
