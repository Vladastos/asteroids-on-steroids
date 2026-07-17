//! Config models (serde) — port of `GameConfig/Models/*.cs` + `GameConfigLoader.cs`.
//! Depends on `serde`/`json5` only; NOT on Bevy. The `game` crate derives Bevy
//! `Component`/`Resource` on wrappers around these, or reads them directly into
//! Bevy resources at startup.

pub mod config;
pub mod loader;

pub use config::GameConfig;
