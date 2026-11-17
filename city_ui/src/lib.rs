//! city_ui — HUD and overlay orchestration (§1.3 Note: full Feathers widget tree is Phase 5).
//!
//! Phase 1: plugin skeleton + debug-overlay state. Real widgets (heatmaps, graphs, toolbars)
//! attach here in Phase 5; keeping the crate compiling now keeps the dependency graph honest.

#![forbid(unsafe_code)]

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::Resource;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HeatmapMode {
    Off,
    LandValue,
    TrafficFlow,
    Happiness,
    ElectricityLoad,
    WaterPressure,
    Pollution,
}

#[derive(Resource, Default)]
pub struct UiState {
    pub debug_overlay: bool,
    pub heatmap: HeatmapMode,
    pub selected_building: Option<city_core::BuildingId>,
}
impl Default for HeatmapMode {
    fn default() -> Self {
        HeatmapMode::Off
    }
}

pub struct CityUiPlugin;

impl Plugin for CityUiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiState>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_heatmap_is_off() {
        assert_eq!(UiState::default().heatmap, HeatmapMode::Off);
    }
}
