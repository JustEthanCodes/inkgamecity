//! RON descriptor pipeline (§5.2). Mods and base content share one descriptor schema.
//! Descriptors are data-only; they materialize into scenes at spawn time.

use serde::Deserialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DescriptorError {
    #[error("ron parse error in `{name}`: {source}")]
    Parse { name: String, source: ron::error::SpannedError },
    #[error("descriptor `{0}` exceeds 1 MiB size cap")]
    TooLarge(String),
}

/// Building definition as authored in `assets/mods/<name>/*.ron` or base content.
#[derive(Clone, Debug, Deserialize)]
pub struct BuildingDef {
    pub id: String,
    pub display_name: String,
    pub footprint: Footprint,
    /// glTF asset paths with optional `#LODn` fragments.
    pub meshes: Vec<String>,
    #[serde(default)]
    pub attach_tags: Vec<String>,
    pub cost_cents: i64,
    pub maintenance_per_day_cents: i64,
    #[serde(default)]
    pub capacity: Option<Capacity>,
    /// Unlock milestone (name, level); `None` = available from start.
    #[serde(default)]
    pub unlock: Option<(String, u8)>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Footprint {
    pub w: f32,
    pub d: f32,
    pub h: f32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Capacity {
    pub residents: u16,
    pub jobs: u16,
}

pub const MAX_DESCRIPTOR_BYTES: usize = 1024 * 1024;

/// Parse a BuildingDef from RON text with the §5.2 size cap enforced.
pub fn parse_building_def(name: &str, ron_text: &str) -> Result<BuildingDef, DescriptorError> {
    if ron_text.len() > MAX_DESCRIPTOR_BYTES {
        return Err(DescriptorError::TooLarge(name.to_string()));
    }
    ron::from_str(ron_text).map_err(|source| DescriptorError::Parse { name: name.to_string(), source })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        (
            id: "core.office_tower",
            display_name: "Meridian Office Tower",
            footprint: (w: 28.0, d: 28.0, h: 96.0),
            meshes: ["buildings/office_tower.glb#LOD0", "buildings/office_tower_lod1.glb#LOD1"],
            attach_tags: ["rooftop_hvac"],
            capacity: Some((residents: 0, jobs: 1400)),
            cost_cents: 9_600_000_000,
            maintenance_per_day_cents: 1_800_000,
            unlock: Some(("SkylineCharter", 6)),
        )
    "#;

    #[test]
    fn parses_building_def() {
        let def = parse_building_def("sample", SAMPLE).unwrap();
        assert_eq!(def.id, "core.office_tower");
        assert_eq!(def.capacity.unwrap().jobs, 1400);
        assert_eq!(def.unlock.unwrap().1, 6);
    }

    #[test]
    fn rejects_malformed_ron() {
        assert!(parse_building_def("bad", "(id: ").is_err());
    }

    #[test]
    fn enforces_size_cap() {
        let big = "x".repeat(MAX_DESCRIPTOR_BYTES + 1);
        assert!(matches!(parse_building_def("big", &big), Err(DescriptorError::TooLarge(_))));
    }
}
