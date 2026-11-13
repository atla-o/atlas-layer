#![forbid(unsafe_code)]

mod runtime;

pub mod config;
pub mod connectors;
pub mod history;
pub mod inbox;
pub mod model;
pub mod open;
pub mod secrets;
pub mod services;
pub mod todos;

pub fn run() {
    runtime::run();
}
