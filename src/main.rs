use tidal_dl_ng::cli::app;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    app::run().await
}
