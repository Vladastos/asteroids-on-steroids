//! Components + config models (serde). Port of Engine/Components + GameConfig.
//! Depends on the pure crates + glam; NOT on Bevy. The `game` crate derives
//! Bevy `Component` on wrappers around these, or #[derive(Component)] directly
//! once you add a `bevy_ecs` dep here.
