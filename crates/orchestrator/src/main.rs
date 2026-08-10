//! Ghostlight 1.0 persistent orchestrator service process.

fn main() -> anyhow::Result<()> {
    ghostlight::service::run_forever()
}
