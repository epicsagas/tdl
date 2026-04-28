use tdl::cli::app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = tdl::logging::init();
    app::run().await
}
