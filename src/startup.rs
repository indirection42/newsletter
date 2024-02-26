use crate::config::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::{health_check, subscribe};
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::io;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

// A new type to exposure the server local port
pub struct Application {
    server: Server,
    port: u16,
}

impl Application {
    pub async fn build(config: Settings) -> Result<Self, io::Error> {
        let conn_pool = get_conn_pool(&config.database);

        let sender_email = config
            .email_client
            .sender_email
            .parse()
            .expect("Invalid sender email.");

        let timeout = config.email_client.timeout();

        let email_client = EmailClient::new(
            config.email_client.base_url,
            sender_email,
            config.email_client.authorization_token,
            timeout,
        );

        let address = format!("{}:{}", config.application.host, config.application.port);

        let listener = TcpListener::bind(address)?;

        let port = listener.local_addr().unwrap().port();
        let server = run(listener, conn_pool, email_client)?;
        Ok(Application { server, port })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), io::Error> {
        self.server.await
    }
}

pub fn get_conn_pool(config: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(config.with_db())
}

pub fn run(
    listener: TcpListener,
    conn_pool: PgPool,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    let email_client = web::Data::new(email_client);
    let conn_pool = web::Data::new(conn_pool);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check))
            .route("/subscriptions", web::post().to(subscribe))
            .app_data(conn_pool.clone())
            .app_data(email_client.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
