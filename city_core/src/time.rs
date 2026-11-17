//! Tick arithmetic and simulation speed. One tick is one game-minute at 60 Hz sim rate.

use serde::{Deserialize, Serialize};

/// Fixed simulation rate of the deterministic lane (see Section 3 of the design doc).
pub const SIM_HZ: f64 = 60.0;
/// Game-time seconds advanced per simulation tick.
pub const GAME_SECONDS_PER_TICK: f32 = 60.0;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tick(pub u64);

impl Tick {
    pub const ZERO: Self = Self(0);
    #[inline]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Debug for Tick {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Tick({})", self.0)
    }
}

impl std::ops::Add<u64> for Tick {
    type Output = Tick;
    fn add(self, rhs: u64) -> Tick {
        Tick(self.0 + rhs)
    }
}

/// Wall-speed selection. `Ultra` = 4× fixed steps per frame; physics never changes dt.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SimSpeed {
    Paused,
    #[default]
    Normal,
    Fast,
    Ultra,
}

impl SimSpeed {
    #[inline]
    pub const fn steps_per_frame(self) -> u32 {
        match self {
            SimSpeed::Paused => 0,
            SimSpeed::Normal => 1,
            SimSpeed::Fast => 2,
            SimSpeed::Ultra => 4,
        }
    }
}
