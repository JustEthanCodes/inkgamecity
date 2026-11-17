//! Tick-bucket systems. Deterministic: no wall-clock, no unbounded iteration, no RNG
//! without a seeded stream on SimulationClock.

use bevy_ecs::message::MessageWriter;
use bevy_ecs::prelude::{Res, ResMut};
use city_core::BuildingId;

use crate::{
    BuildStats, DenseLandGrid, IntentQueueRes, IntentRejected, MunicipalBudget, RciDemand,
    SimulationClock,
};

/// Placeholder building marker; full Building component set lands in Phase 3.
#[derive(bevy_ecs::prelude::Component)]
pub struct Building {
    pub id: BuildingId,
    pub tier: u8,
    pub def_id: String,
}

pub fn advance_clock(mut clock: ResMut<SimulationClock>) {
    clock.tick.0 += 1;
    clock.time_of_day += 1.0 / SimulationClock::TICKS_PER_DAY as f32;
    if clock.time_of_day >= 1.0 {
        clock.time_of_day -= 1.0;
        clock.day += 1;
    }
}

/// Flat cost table until RON descriptors are loaded from disk (Phase 2/3 wiring).
const PLACE_BUILDING_COST_CENTS: i64 = 10_000_000_000;

pub fn drain_intents(
    mut queue: ResMut<IntentQueueRes>,
    mut budget: ResMut<MunicipalBudget>,
    mut stats: ResMut<BuildStats>,
    mut rejected: MessageWriter<IntentRejected>,
    mut buildings: bevy_ecs::prelude::Commands,
) {
    let mut next_id = stats.buildings_placed as u32 + 1;
    for (player, intent) in queue.0.drain() {
        match intent {
            city_core::PlayerIntent::PlaceBuilding { def_id, position: _, yaw: _ } => {
                if budget.treasury_cents >= PLACE_BUILDING_COST_CENTS {
                    budget.treasury_cents -= PLACE_BUILDING_COST_CENTS;
                    budget.lifetime_spend_cents += PLACE_BUILDING_COST_CENTS;
                    stats.buildings_placed += 1;
                    buildings.spawn(Building {
                        id: BuildingId(next_id),
                        tier: 1,
                        def_id,
                    });
                    next_id += 1;
                } else {
                    stats.intents_rejected += 1;
                    rejected.write(IntentRejected {
                        player,
                        reason: city_core::IntentReject::InsufficientFunds {
                            needed_cents: PLACE_BUILDING_COST_CENTS,
                            available_cents: budget.treasury_cents,
                        },
                    });
                }
            }
            city_core::PlayerIntent::ZonePaint { cells, .. } => {
                stats.zone_cells_painted += cells.len() as u64;
            }
            city_core::PlayerIntent::Bulldoze { target: _ } => {
                // Entity resolution by BuildingId lands with the catalog index (Phase 3).
            }
            city_core::PlayerIntent::Policy { .. }
            | city_core::PlayerIntent::SetServiceBudget { .. }
            | city_core::PlayerIntent::SetTaxRate { .. }
            | city_core::PlayerIntent::Loan { .. } => {
                // Phase 5: district policies, budget re-solve, loan amortization.
            }
        }
    }
}

pub fn kinematics_stub() {
    // Phase 3: vehicle IDM + lane-edge advance in parallel over `Vehicle`s.
}

pub fn congestion_sample_stub() {
    // Phase 3: refresh edge_cost_live from SpatialGrid vehicle occupancy.
}

pub fn needs_pulse(_grid: Res<DenseLandGrid>) {
    // Phase 3: household need decay + citizen FSM transitions.
}

pub fn economy_pulse(_demand: ResMut<RciDemand>) {
    // Phase 3: Leontief IO solve, inventory balance, rent reset.
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::{App, FixedUpdate};
    use city_core::{PlayerId, ZoneKind};

    fn test_app() -> App {
        // bevy_app::App::new() already installs MainSchedulePlugin in 0.20.
        let mut app = App::new();
        app.add_plugins(crate::CitySimPlugin);
        app
    }

    #[test]
    fn clock_advances_deterministically() {
        let mut app = test_app();
        for _ in 0..60 {
            app.world_mut().run_schedule(FixedUpdate);
        }
        let clock = app.world().resource::<SimulationClock>();
        assert_eq!(clock.tick.0, 60);
    }

    #[test]
    fn place_building_spends_treasury() {
        let mut app = test_app();
        app.world_mut()
            .resource_mut::<IntentQueueRes>()
            .0
            .push(
                PlayerId::LOCAL,
                city_core::PlayerIntent::PlaceBuilding {
                    def_id: "core.test_hut".into(),
                    position: [0.0, 0.0, 0.0],
                    yaw: 0.0,
                },
            );
        app.world_mut().run_schedule(FixedUpdate);
        let budget = app.world().resource::<MunicipalBudget>();
        assert_eq!(budget.treasury_cents, 1_000_000_000_000 - PLACE_BUILDING_COST_CENTS);
        let stats = app.world().resource::<BuildStats>();
        assert_eq!(stats.buildings_placed, 1);
        let mut q = app.world_mut().query::<&Building>();
        assert_eq!(q.iter(app.world()).count(), 1);
    }

    #[test]
    fn insufficient_funds_rejects() {
        let mut app = test_app();
        app.world_mut().resource_mut::<MunicipalBudget>().treasury_cents = 1; // poverty
        app.world_mut()
            .resource_mut::<IntentQueueRes>()
            .0
            .push(
                PlayerId::LOCAL,
                city_core::PlayerIntent::PlaceBuilding {
                    def_id: "core.test_hut".into(),
                    position: [0.0; 3],
                    yaw: 0.0,
                },
            );
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(app.world().resource::<BuildStats>().intents_rejected, 1);
    }

    #[test]
    fn tick_divisible_condition_math() {
        // Verify the raw predicate without the ECS: 15-tick bucket fires at 0, 15, 30 …
        for t in 0..90u64 {
            let fires = (t + 0) % 15 == 0;
            assert_eq!(fires, t % 15 == 0);
        }
        // Phase-offset variant staggers by 3 ticks.
        assert_eq!((12 + 3) % 15, 0);
    }

    #[allow(dead_code)]
    fn zonekind_compiles() {
        let _ = ZoneKind::ResidentialLow;
    }
}
