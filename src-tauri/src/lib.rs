mod error;
pub mod commands;

#[cfg(not(test))]
mod app;

#[cfg(not(test))]
pub fn run() {
    app::run();
}