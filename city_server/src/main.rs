//! cityfloorlines-server — headless simulation runner (§7 dedicated topology).
//!
//! Usage: cityfloorlines-server [--ticks N] [--seed S]
//!
//! Runs the deterministic sim without a window/render stack and prints a one-line status
//! per game-day plus the final world hash (§7.8). This is the entry point for:
//!   * CI determinism harness (same seed → same hash on every platform),
//!   * soak tests (72 h uninterrupted sim, NaN sweep included),
//!   * the dedicated-server phase of Phase 7.

use bevy_app::{App, FixedUpdate};
use bevy_time::TimePlugin;
use city_core::SimSpeed;
use city_sim::{BuildStats, CitySimPlugin, DenseLandGrid, MunicipalBudget, SimulationClock};
use city_net::world_hash;

fn main() {
    let mut ticks: u64 = 600; // default: 10 game-hours at 1 min/tick
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ticks" => ticks = args.next().and_then(|v| v.parse().ok()).unwrap_or(ticks),
            "--help" | "-h" => {
                println!("cityfloorlines-server [--ticks N] [--seed S]");
                return;
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }

    let mut app = App::new();
    app.add_plugins(TimePlugin);
    app.add_plugins(CitySimPlugin);
    app.world_mut().resource_mut::<SimulationClock>().speed = SimSpeed::Normal;

    let mut last_day = 0u32;
    for _ in 0..ticks {
        app.world_mut().run_schedule(FixedUpdate);
        let day = app.world().resource::<SimulationClock>().day;
        if day != last_day {
            last_day = day;
            let stats = app.world().resource::<BuildStats>();
            let budget = app.world().resource::<MunicipalBudget>();
            println!("day {day}: buildings={} treasury_cents={}", stats.buildings_placed, budget.treasury_cents);
        }
    }

    let clock = app.world().resource::<SimulationClock>();
    let stats = app.world().resource::<BuildStats>();
    let budget = app.world().resource::<MunicipalBudget>();
    let pollution = app
        .world()
        .get_resource::<DenseLandGrid>()
        .map(|_| 0u64) // wired to the real pollution grid checksum in Phase 3/6
        .unwrap_or(0);
    let hash = world_hash(clock.tick.0, budget.treasury_cents, stats.buildings_placed, pollution);
    println!(
        "final: tick={} day={} buildings={} treasury={} world_hash={hash:#018x}",
        clock.tick.0, clock.day, stats.buildings_placed, budget.treasury_cents
    );
}
