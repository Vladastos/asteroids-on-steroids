//! Port of `GameConfig/Models/ShapeData.cs`.

use serde::Deserialize;

/// Authored compound-body shape exported from the shape editor. Coordinates
/// are centroid-normalised (centroid at `[0,0]`).
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShapeData {
    pub name: String,
    pub material: String,
    /// Outline polygon vertices as `[x, y]` pairs.
    pub outline: Vec<[f32; 2]>,
    pub seeds: Vec<SeedData>,
}

impl Default for ShapeData {
    fn default() -> Self {
        Self {
            name: String::new(),
            material: "metal".into(),
            outline: Vec::new(),
            seeds: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeedData {
    pub x: f32,
    pub y: f32,
    #[serde(default = "default_role")]
    pub role: RoleTag,
    #[serde(default = "one")]
    pub bond_mult: f32,
    #[serde(default = "one")]
    pub density_mult: f32,
}

/// Placeholder for the `role` string (kept as a plain string to avoid baking in
/// an exhaustive enum the shape editor might extend). See `RoleTag` doc.
pub type RoleTag = String;
fn default_role() -> RoleTag {
    "generic".into()
}
fn one() -> f32 {
    1.0
}
