//! Resources for the simulation lane. Components live in `systems`/separate modules as they land.

use bevy_ecs::prelude::Resource;
use city_core::{ChunkedGrid, SimSpeed, Tick};

#[derive(Resource)]
pub struct SimulationClock {
    pub tick: Tick,
    /// 0..1 fraction of a game day elapsed.
    pub time_of_day: f32,
    pub day: u32,
    pub speed: SimSpeed,
}

impl Default for SimulationClock {
    fn default() -> Self {
        Self { tick: Tick::ZERO, time_of_day: 0.30, day: 1, speed: SimSpeed::Normal }
    }
}

impl SimulationClock {
    pub const GAME_MINUTES_PER_TICK: f32 = 1.0;
    pub const TICKS_PER_DAY: u64 = 24 * 60;
}

#[derive(Resource)]
pub struct IntentQueueRes(pub city_core::IntentQueue);

impl Default for IntentQueueRes {
    fn default() -> Self {
        Self(city_core::IntentQueue::default())
    }
}

#[derive(Resource, Clone, Default)]
pub struct MunicipalLoan {
    pub amount_cents: i64,
    pub annual_rate_bp: u32,
    pub remaining_ticks: u32,
}

#[derive(Resource)]
pub struct MunicipalBudget {
    pub treasury_cents: i64,
    pub lifetime_tax_cents: i64,
    pub lifetime_spend_cents: i64,
    pub service_funding: [u8; city_core::SERVICE_COUNT],
    pub loans: Vec<MunicipalLoan>,
}

impl Default for MunicipalBudget {
    fn default() -> Self {
        Self {
            treasury_cents: 1_000_000_000_000, // 1.0M in cents ×100 → starter city grant
            lifetime_tax_cents: 0,
            lifetime_spend_cents: 0,
            service_funding: [128; city_core::SERVICE_COUNT],
            loans: Vec::new(),
        }
    }
}

#[derive(Resource, Default)]
pub struct RciDemand {
    pub residential: f32,
    pub commercial: f32,
    pub industrial: f32,
    pub office: f32,
}

#[derive(Resource, Default)]
pub struct BuildStats {
    pub buildings_placed: u64,
    pub zone_cells_painted: u64,
    pub intents_rejected: u64,
}

#[derive(Resource)]
pub struct DenseLandGrid(pub ChunkedGrid);

impl Default for DenseLandGrid {
    fn default() -> Self {
        Self(ChunkedGrid::new(512, 512, 16.0)) // 8 km × 8 km @ 16 m cells
    }
}
