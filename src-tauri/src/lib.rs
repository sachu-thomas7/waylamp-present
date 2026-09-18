mod error;
pub mod commands;
pub mod events;
pub mod bus;
pub mod config;
pub mod config_loader;
pub mod slide;
pub mod slide_state_machine;
pub mod scene;
pub mod deck_importer;

#[cfg(not(test))]
mod app;

#[cfg(not(test))]
pub fn run() {
    app::run();
}