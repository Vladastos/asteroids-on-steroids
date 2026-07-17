//! Config/shape loading — port of `GameConfig/GameConfigLoader.cs` (load side;
//! save/editor support is out of scope for the runtime port).
//!
//! `json5` stands in for `System.Text.Json`'s `ReadCommentHandling.Skip` +
//! `AllowTrailingCommas = true`, so `Assets/*.json` load unchanged.
//!
//! Native reads straight off disk. wasm (Phase 6) has no filesystem — swap
//! `load_from_str`/`load_shape_from_str` in behind an `include_str!` or
//! `AssetServer` fetch; the parsing logic here is unchanged either way.

use crate::config::{GameConfig, ShapeData};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "io error: {e}"),
            LoadError::Parse(e) => write!(f, "parse error: {e}"),
        }
    }
}
impl std::error::Error for LoadError {}

/// Loads `game_config.json` and all shape files from `assets_dir`. Shapes are
/// keyed by filename stem (e.g. "player_ship" for player_ship.json).
pub fn load(assets_dir: &Path) -> Result<(GameConfig, HashMap<String, ShapeData>), LoadError> {
    let config_path = assets_dir.join("game_config.json");
    let text = std::fs::read_to_string(&config_path).map_err(LoadError::Io)?;
    let config = load_config_from_str(&text)?;
    let shapes = load_shapes(&assets_dir.join("shapes"));
    Ok((config, shapes))
}

pub fn load_config_from_str(text: &str) -> Result<GameConfig, LoadError> {
    json5::from_str(text).map_err(|e| LoadError::Parse(e.to_string()))
}

pub fn load_shape_from_str(text: &str) -> Result<ShapeData, LoadError> {
    json5::from_str(text).map_err(|e| LoadError::Parse(e.to_string()))
}

/// Loads every `*.json` in `shapes_dir`, keyed by filename stem. Malformed
/// files are skipped (logged to stderr), matching the C#'s permissive loader —
/// one bad shape file shouldn't crash the whole load.
pub fn load_shapes(shapes_dir: &Path) -> HashMap<String, ShapeData> {
    let mut result = HashMap::new();
    let Ok(entries) = std::fs::read_dir(shapes_dir) else {
        return result;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        match std::fs::read_to_string(&path)
            .map_err(LoadError::Io)
            .and_then(|t| load_shape_from_str(&t))
        {
            Ok(shape) => {
                result.insert(stem, shape);
            }
            Err(e) => eprintln!(
                "[loader] skipping malformed shape '{}': {e}",
                path.display()
            ),
        }
    }
    result
}

/// Walks up from `start_dir` until it finds a sibling folder named "Assets".
/// Port of `GameConfigLoader.FindAssetsDir`.
pub fn find_assets_dir(start_dir: &Path) -> Option<PathBuf> {
    let mut dir = std::fs::canonicalize(start_dir).ok();
    while let Some(d) = dir {
        let candidate = d.join("Assets");
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = d.parent().map(|p| p.to_path_buf());
    }
    None
}
