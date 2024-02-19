use sqlx::postgres::PgPoolOptions;
use std::net::TcpListener;
use zero2prod::config::get_config;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subsciber, init_subscriber};
#[tokio::main]
async fn main() -> std::io::Result<()> {
    let subscriber = get_subsciber("zero2prod".into(), "info".into(), std::io::stdout);
    init_subscriber(subscriber);
    let config = get_config().expect("Failed to read configuration.");
    let conn = PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(config.database.with_db());
    let address = format!("{}:{}", config.application.host, config.application.port);
    let listener = TcpListener::bind(address)?;
    run(listener, conn)?.await
}
