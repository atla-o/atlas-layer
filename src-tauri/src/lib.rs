#![forbid(unsafe_code)]

mod runtime;

pub mod compute;
pub mod config;
pub mod connectors;
pub mod filter;
pub mod history;
pub mod inbox;
pub mod model;
pub mod secrets;
pub mod services;
pub mod todos;

pub fn run() {
    runtime::run();
}
