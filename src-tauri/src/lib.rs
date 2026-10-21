pub mod bus;
pub mod commands;
pub mod compositor;
pub mod config;
pub mod config_loader;
pub mod deck_importer;
pub mod display;
pub mod error;
pub mod events;
pub mod ndi;
pub mod pipeline;
pub mod preview;
pub mod scene;
pub mod slide;
pub mod slide_state_machine;
pub mod state;
pub mod whip;
pub mod whip_server;

#[cfg(not(test))]
mod app;

#[cfg(not(test))]
pub fn run() {
    app::run();
}
