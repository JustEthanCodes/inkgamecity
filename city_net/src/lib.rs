//! city_net — replication contract for multiplayer (§7).
//!
//! This crate defines *what* crosses the wire and how interest management accounts tiles;
//! the socket layer (lightyear/bevy_replicon) is integrated in Phase 7 and deliberately
//! not stubbed with half-working networking here.
//!
//! The server is the only simulation owner. Clients are thin: render + UI + intents.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

/// Replicated snapshot kinds. Anything not listed here never leaves the server.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProxyKind {
    Building,
    Vehicle,
    Pedestrian,
    ZoneTile,
    GridSlice,
    BudgetDigest,
    Alert,
}

/// Interest management tiles are 128 m — sub-cell of the 1024 m terrain chunk (§4).
pub const INTEREST_TILE_M: f32 = 128.0;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileCoord {
    pub x: i32,
    pub y: i32,
}

impl TileCoord {
    #[inline]
    pub fn from_world(p: (f32, f32)) -> Self {
        Self { x: (p.0 / INTEREST_TILE_M).floor() as i32, y: (p.1 / INTEREST_TILE_M).floor() as i32 }
    }

    /// Static interest region for a camera position (street zoom). Radial upgrade lands
    /// with the zoom-aware version in Phase 7.
    pub fn interest_set(center: TileCoord, radius_tiles: i32) -> Vec<TileCoord> {
        let r = radius_tiles.max(0);
        let mut out = Vec::with_capacity(((2 * r + 1) * (2 * r + 1)) as usize);
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r {
                    out.push(TileCoord { x: center.x + dx, y: center.y + dy });
                }
            }
        }
        out
    }
}

/// FNV-1a divergence hash (§7.8). Cheap; server broadcasts at 0.2 Hz on the debug channel.
pub fn world_hash(tick: u64, treasury_cents: i64, population: u64, pollution_checksum: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut mix = |bytes: &[u8]| {
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    };
    mix(&tick.to_le_bytes());
    mix(&treasury_cents.to_le_bytes());
    mix(&population.to_le_bytes());
    mix(&pollution_checksum.to_le_bytes());
    h
}

/// Snapshot cadence policy: adaptive drop-to-6 Hz under load (§7.11).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct SnapshotCadence {
    pub normal_every_n_ticks: u64,
    pub degraded_every_n_ticks: u64,
    pub degraded: bool,
}

impl Default for SnapshotCadence {
    fn default() -> Self {
        Self { normal_every_n_ticks: 6, degraded_every_n_ticks: 10, degraded: false }
    }
}

impl SnapshotCadence {
    #[inline]
    pub fn effective_period(&self) -> u64 {
        if self.degraded { self.degraded_every_n_ticks } else { self.normal_every_n_ticks }
    }
}

/// Whitelist check used in tests: component type names that may replicate.
pub const REPLICATED_TYPES: &[&str] = &[
    "Building",
    "RenderProxy",
    "Transform",
    "Vehicle",
    "ReplicatedProxy",
];

pub fn is_replicated(type_name: &str) -> bool {
    REPLICATED_TYPES.contains(&type_name)
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentChannel {
    /// Ordered, reliable — zones, builds, budget, policies.
    Ordered,
    /// Unreliable-sequenced — presence cursor, camera ping, emotes.
    Sequenced,
}

/// One client's view of the world budget (§7.6): builds/min, zone cells/s, policy/min.
#[derive(Copy, Clone, Debug)]
pub struct RateLimits {
    pub builds_per_min: u32,
    pub zone_cells_per_sec: u32,
    pub policies_per_min: u32,
}

impl Default for RateLimits {
    fn default() -> Self {
        Self { builds_per_min: 60, zone_cells_per_sec: 400, policies_per_min: 10 }
    }
}

#[derive(Clone, Debug)]
pub struct IntentValidationContext {
    pub limits: RateLimits,
    pub recent_builds: SmallVec<[u64; 64]>, // ticks of recent build intents (window 3600)
}

impl Default for IntentValidationContext {
    fn default() -> Self {
        Self { limits: RateLimits::default(), recent_builds: SmallVec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interest_set_area_and_radius() {
        let set = TileCoord::interest_set(TileCoord { x: 0, y: 0 }, 2);
        // Circle of radius 2: count = π·4 ≈ 13 cells.
        assert!((12..=14).contains(&set.len()), "len = {}", set.len());
        assert!(set.contains(&TileCoord { x: 0, y: 0 }));
        assert!(!set.contains(&TileCoord { x: 2, y: 2 })); // outside circle (8 > 4)
    }

    #[test]
    fn world_hash_changes_with_any_field() {
        let a = world_hash(100, 5000, 9000, 123);
        assert_ne!(a, world_hash(101, 5000, 9000, 123));
        assert_ne!(a, world_hash(100, 5001, 9000, 123));
        assert_ne!(a, world_hash(100, 5000, 9001, 123));
        assert_ne!(a, world_hash(100, 5000, 9000, 124));
    }

    #[test]
    fn whitelist_blocks_sim_internals() {
        assert!(is_replicated("Building"));
        assert!(!is_replicated("Household"));
        assert!(!is_replicated("Company"));
        assert!(!is_replicated("Citizen"));
    }

    #[test]
    fn cadence_degrades_only_when_flagged() {
        let mut c = SnapshotCadence::default();
        assert_eq!(c.effective_period(), 6);
        c.degraded = true;
        assert_eq!(c.effective_period(), 10);
    }

    #[test]
    fn tile_from_world_floors_correctly() {
        assert_eq!(TileCoord::from_world((0.0, 0.0)), TileCoord { x: 0, y: 0 });
        assert_eq!(TileCoord::from_world((127.9, -0.1)), TileCoord { x: 0, y: -1 });
        assert_eq!(TileCoord::from_world((256.0, 384.0)), TileCoord { x: 2, y: 3 });
    }
}
