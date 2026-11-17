//! Player → simulation intent pipeline. Both single-player and multiplayer flow through
//! this queue, so UI code never writes simulation state directly.
//!
//! Server-side validation happens in `city_sim::systems::intents` against `MunicipalBudget`
//! and district permissions — identical code path headless or online.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use crate::{BuildingId, DistrictId, PlayerId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlayerIntent {
    /// Paint zone cells. `cells` are dense chunked-grid coordinates.
    ZonePaint { cells: SmallVec<[u32; 32]>, zone: crate::ZoneKind },
    /// Place a building by definition id (RON descriptor, e.g. "core.office_tower").
    PlaceBuilding { def_id: String, position: [f32; 3], yaw: f32 },
    /// Demolish a building.
    Bulldoze { target: BuildingId },
    /// Toggle a district policy.
    Policy { district: DistrictId, policy_id: String, enabled: bool },
    /// Set a municipal service funding level (0-255).
    SetServiceBudget { service: crate::Service, level: u8 },
    /// Set residential tax rate in basis points for a tier; 100 bp = 1%.
    SetTaxRate { tier: u8, rate_bp: u16 },
    /// Take a municipal loan.
    Loan { amount_cents: i64, term_ticks: u32 },
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentReject {
    UnknownBuildingDef,
    InsufficientFunds { needed_cents: i64, available_cents: i64 },
    InvalidCells,
    OutOfBounds,
    RateLimited,
    Unauthorized,
}

/// FIFO queue drained once per FixedUpdate tick. Engine-free so servers can drive it headless.
#[derive(Default)]
pub struct IntentQueue {
    intents: std::collections::VecDeque<(PlayerId, PlayerIntent)>,
}

impl IntentQueue {
    pub fn push(&mut self, player: PlayerId, intent: PlayerIntent) {
        self.intents.push_back((player, intent));
    }

    pub fn drain(&mut self) -> impl Iterator<Item = (PlayerId, PlayerIntent)> + '_ {
        self.intents.drain(..)
    }

    pub fn len(&self) -> usize {
        self.intents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }
}
