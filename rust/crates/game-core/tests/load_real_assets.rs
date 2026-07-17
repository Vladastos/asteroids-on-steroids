//! Loads the REAL `GameEngine/Assets/*.json` files the C# game ships with and
//! checks known values round-trip — the payoff of a Bevy-free config crate:
//! this is testable with zero engine/renderer setup, and it's the strongest
//! signal that the port's field names/defaults actually match production data.

use game_core::loader;
use std::path::PathBuf;

fn assets_dir() -> PathBuf {
    // rust/crates/game-core -> rust/crates -> rust -> repo root -> GameEngine/Assets
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for _ in 0..3 {
        dir.pop();
    }
    dir.join("GameEngine").join("Assets")
}

#[test]
fn loads_the_real_game_config_with_expected_values() {
    let dir = assets_dir();
    let (config, shapes) = loader::load(&dir).expect("game_config.json + shapes must load");

    // Spot-check values straight from GameEngine/Assets/game_config.json.
    let rock = config
        .materials
        .get("rock")
        .expect("rock material must be present");
    assert!((rock.toughness - 38.0).abs() < 1e-6);
    assert!((rock.brittleness - 0.5).abs() < 1e-6);
    assert!((rock.crack_directionality - 0.75).abs() < 1e-6);

    assert!(config.materials.contains_key("ice"));
    assert!(config.materials.contains_key("metal"));

    // At least one shape file (bruiser.json etc.) must have loaded.
    assert!(
        !shapes.is_empty(),
        "expected shape files under Assets/shapes to load"
    );
    if let Some(bruiser) = shapes.get("bruiser") {
        assert_eq!(bruiser.name, "bruiser");
        assert!(!bruiser.outline.is_empty());
        assert!(!bruiser.seeds.is_empty());
    }
}

#[test]
fn every_shape_file_parses_without_error() {
    let dir = assets_dir().join("shapes");
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("expected {} to exist: {e}", dir.display()))
        .flatten()
        .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    assert!(!entries.is_empty(), "expected at least one shape file");

    let shapes = loader::load_shapes(&dir);
    assert_eq!(
        shapes.len(),
        entries.len(),
        "every shape file in Assets/shapes must parse cleanly"
    );
}

#[test]
fn missing_fields_fall_back_to_defaults_not_errors() {
    // A minimal, deliberately sparse config — every omitted field must resolve
    // to its Default impl rather than failing to parse.
    let text = r#"{ "maxLiveCells": 42 }"#;
    let config = loader::load_config_from_str(text).expect("sparse config must still parse");
    assert_eq!(config.max_live_cells, 42);
    assert!(
        (config.player.thrust - 4500.0).abs() < 1e-6,
        "omitted player config must use its default"
    );
    assert!(config.materials.is_empty());
}

#[test]
fn json5_comments_and_trailing_commas_are_accepted() {
    // Mirrors System.Text.Json's ReadCommentHandling.Skip + AllowTrailingCommas.
    let text = r#"{
        // a comment
        "maxLiveCells": 7,
    }"#;
    let config =
        loader::load_config_from_str(text).expect("comments + trailing commas must be accepted");
    assert_eq!(config.max_live_cells, 7);
}
