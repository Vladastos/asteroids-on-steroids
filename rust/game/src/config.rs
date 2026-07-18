use bevy::{log::info, prelude::*};
use fracture::FractureProperties;
use game_core::{
    config::{MaterialConfig, ShapeData},
    loader, GameConfig,
};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[derive(Resource)]
pub struct GameConfigRes(pub GameConfig);

#[derive(Resource)]
pub struct ShapeLibrary(pub HashMap<String, ShapeData>);

#[derive(Resource, Debug, Clone)]
pub struct AssetsDir(pub PathBuf);

pub fn load_game_config(mut commands: Commands) {
    let assets_dir = locate_assets_dir().unwrap_or_else(|| {
        panic!(
            "could not locate GameEngine/Assets from current_dir={:?} current_exe={:?}",
            std::env::current_dir(),
            std::env::current_exe()
        )
    });
    let (config, shapes) = loader::load(&assets_dir)
        .unwrap_or_else(|err| panic!("failed to load assets from {:?}: {err}", assets_dir));

    info!(
        "loaded {} materials, {} shapes from {:?}",
        config.materials.len(),
        shapes.len(),
        assets_dir
    );

    commands.insert_resource(AssetsDir(assets_dir));
    commands.insert_resource(GameConfigRes(config));
    commands.insert_resource(ShapeLibrary(shapes));
}

pub fn material_to_fracture_properties(m: &MaterialConfig) -> FractureProperties {
    FractureProperties {
        brittleness: m.brittleness,
        toughness: m.toughness,
        restitution: m.restitution,
        relax_rate: m.relax_rate,
        crack_directionality: m.crack_directionality,
        crack_speed: m.crack_speed,
        grain_area: m.grain_area,
        min_fragment_area: m.min_fragment_area,
        density: m.density,
        cell_toughness: m.cell_toughness,
        spin_pre_stress: m.spin_pre_stress,
        detach_cell_scale: m.detach_cell_scale,
        detach_cell_jitter: m.detach_cell_jitter,
    }
}

fn locate_assets_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .and_then(|dir| loader::find_assets_dir(&dir))
        .or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|dir| loader::find_assets_dir(&dir))
        })
        .or_else(manifest_relative_assets_dir)
}

fn manifest_relative_assets_dir() -> Option<PathBuf> {
    let game_manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidate = game_manifest_dir
        .parent()
        .and_then(Path::parent)
        .map(|repo_root| repo_root.join("GameEngine").join("Assets"))?;

    candidate.is_dir().then_some(candidate)
}
