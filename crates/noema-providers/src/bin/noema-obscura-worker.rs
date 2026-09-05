//! Process-isolated Obscura browser worker.

fn main() {
    let status = noema_providers::run_browser_worker_if_requested().unwrap_or(2);
    std::process::exit(status);
}
