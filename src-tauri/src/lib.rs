mod error;
pub mod commands;
pub mod events;
pub mod bus;
pub mod config;
pub mod config_loader;

#[cfg(not(test))]
mod app;

#[cfg(not(test))]
pub fn run() {
    app::run();
}