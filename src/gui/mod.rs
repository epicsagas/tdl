#[cfg(feature = "gui")]
mod app;

#[cfg(feature = "gui")]
pub use app::run_gui;
