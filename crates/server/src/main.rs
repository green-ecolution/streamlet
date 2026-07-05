use server::config::Settings;
use server::startup::Application;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();
    let settings = Settings::from_env();
    Application::build(&settings).await?.run().await
}
