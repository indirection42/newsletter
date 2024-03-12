use zero2prod::config::get_config;
use zero2prod::startup::Application;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = get_subscriber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);

    let config = get_config().expect("Failed to read configuration.");

    let application = Application::build(config).await?;
    application.run_until_stopped().await?;
    Ok(())
}
