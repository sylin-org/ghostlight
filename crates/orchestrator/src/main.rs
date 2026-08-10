//! Ghostlight 1.0 orchestrator and integrated desktop workbench process.

fn main() -> anyhow::Result<()> {
    if std::env::args_os().any(|argument| argument == "--headless") {
        ghostlight::service::run_forever()
    } else {
        ghostlight::desktop::run()
    }
}
