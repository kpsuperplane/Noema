//! Local HTTP server and development entrypoints for Noema.

mod web;
mod web_server;

pub use web_server::run_daemon_web;
