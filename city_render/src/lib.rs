//! city_render — quality tiers and render orchestration (§1.4, §4.5).
//!
//! This vertical slice implements the tier system and telemetry only. The meshlet/Solari
//! passes are Phase 4 scope; they land behind the `solari`/`meshlets` cargo features.

#![forbid(unsafe_code)]

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::Resource;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum QualityTier {
    /// CSM fallback, no Solari — GTX 1650 / Steam Deck class.
    Potato,
    /// CSM + contact shadows + atmosphere — default for mid GPUs.
    #[default]
    Standard,
    /// Solari GI (bounce 2), DLSS balance mode.
    High,
    /// Solari GI (bounce 3) + DLSS-RR + meshlet path, RT required.
    Ultra,
}

#[derive(Resource, Default)]
pub struct RenderSettings {
    pub quality: QualityTier,
    /// 0.0 = clear day, 1.0 = downpour. Read by WeatherMaterial and (Phase 4) the accident model.
    pub precipitation: f32,
    /// Wetness of road surfaces 0..1 — drives specular response and traffic hazard rate.
    pub road_wetness: f32,
    /// EV100-calibrated exposure target (physically based day/night).
    pub ev100: f32,
}

pub struct CityRenderPlugin;

impl Plugin for CityRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderSettings>()
            .add_systems(Update, update_exposure_ev100);
    }
}

/// Smooth EV100 across the day/night cycle: noon sun ≈ EV15, overcast ≈ EV13, moonlit ≈ EV−3.
/// Full parameterization with atmosphere transmittance is Phase 4; this keeps terrain readable today.
fn update_exposure_ev100(mut settings: bevy_ecs::prelude::ResMut<RenderSettings>) {
    // Placeholder curve: constant pleasant daylight until the sun cycle lands (Phase 4).
    settings.ev100 = settings.ev100.clamp(-6.0, 16.0).max(13.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_default_is_standard() {
        let s = RenderSettings::default();
        assert_eq!(s.quality, QualityTier::Standard);
    }
}
