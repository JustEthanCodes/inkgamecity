//! city_sim — deterministic simulation core (§3). No rendering dependencies.
//!
//! Multi-rate scheduler: one 60 Hz `FixedUpdate` lane, sub-buckets via tick-divisor
//! conditions on [`SimulationClock`]. Physics/kinematics never change dt; wall-speed
//! mapping lives in `SimSpeed::steps_per_frame` (host/mayor authority).

#![forbid(unsafe_code)]

pub mod messages;
pub mod resources;
pub mod systems;

use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::prelude::*;

pub use messages::*;
pub use resources::*;

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Tick60 {
    Clock,
    Intents,
    Kinematics,
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Tick15 {
    PathDispatch,
    CongestionSample,
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Tick1 {
    Needs,
    UtilityFlow,
}

#[derive(SystemSet, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Tick05 {
    Economy,
}

/// Run condition: true every `period` ticks with `offset` phase (staggers heavy buckets).
pub fn tick_divisible(period: u64, offset: u64) -> impl FnMut(Res<SimulationClock>) -> bool {
    move |clock: Res<SimulationClock>| (clock.tick.0 + offset) % period == 0
}

pub struct CitySimPlugin;

impl Plugin for CitySimPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SimulationClock>()
            .init_resource::<IntentQueueRes>()
            .init_resource::<MunicipalBudget>()
            .init_resource::<RciDemand>()
            .init_resource::<BuildStats>()
            .init_resource::<DenseLandGrid>()
            .add_message::<IntentRejected>()
            .add_message::<PathRequest>()
            .add_message::<PathResultReady>();

        app.configure_sets(
            FixedUpdate,
            (
                (Tick60::Clock, Tick60::Intents, Tick60::Kinematics).chain(),
                Tick15::PathDispatch.after(Tick60::Kinematics).run_if(tick_divisible(15, 0)),
                Tick15::CongestionSample.after(Tick60::Kinematics).run_if(tick_divisible(15, 3)),
                Tick1::Needs.after(Tick15::CongestionSample).run_if(tick_divisible(60, 0)),
                Tick1::UtilityFlow.after(Tick15::CongestionSample).run_if(tick_divisible(60, 10)),
                Tick05::Economy.after(Tick1::Needs).run_if(tick_divisible(300, 0)),
            ),
        );

        app.add_systems(
            FixedUpdate,
            (
                systems::advance_clock.in_set(Tick60::Clock),
                systems::drain_intents.in_set(Tick60::Intents),
                systems::kinematics_stub.in_set(Tick60::Kinematics),
                systems::congestion_sample_stub.in_set(Tick15::CongestionSample),
                systems::needs_pulse.in_set(Tick1::Needs),
                systems::economy_pulse.in_set(Tick05::Economy),
            ),
        );
    }
}
