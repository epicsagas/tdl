pub mod cli;
pub mod config;
pub mod download;
pub mod metadata;
pub mod pathfmt;
pub mod tidal;
pub mod tui;

#[cfg(feature = "gui")]
pub mod gui;
