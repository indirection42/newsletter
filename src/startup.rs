use crate::config::{DatabaseSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes::{
    confirm, health_check, home, login, login_form, publish_newsletter, subscribe,
};
use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use secrecy::Secret;
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

        let address = format!("{}:{}", config.application.host, config.application.port);

        let listener = TcpListener::bind(address)?;

        let email_client = config.email_client.client();

        let port = listener.local_addr().unwrap().port();
        let server = run(
            listener,
            conn_pool,
            email_client,
            config.application.base_url,
            config.application.hmac_secret,
        )?;
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

// A wrapper type to be distinguished with raw String
#[derive(Clone)]
pub struct ApplicationBaseUrl(pub String);
#[derive(Clone)]
pub struct HmacSecret(pub Secret<String>);
pub fn run(
    listener: TcpListener,
    conn_pool: PgPool,
    email_client: EmailClient,
    confirm_base_url: String,
    hmac_secret: Secret<String>,
) -> Result<Server, std::io::Error> {
    let email_client = web::Data::new(email_client);
    let conn_pool = web::Data::new(conn_pool);
    let base_url = web::Data::new(ApplicationBaseUrl(confirm_base_url));
    let hmac_secret = web::Data::new(HmacSecret(hmac_secret));
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/", web::get().to(home))
            .route("/health_check", web::get().to(health_check))
            .route("/login", web::get().to(login_form))
            .route("/login", web::post().to(login))
            .route("/subscriptions", web::post().to(subscribe))
            .route("/subscriptions/confirm", web::get().to(confirm))
            .route("/newsletters", web::post().to(publish_newsletter))
            .app_data(conn_pool.clone())
            .app_data(email_client.clone())
            .app_data(base_url.clone())
            .app_data(hmac_secret.clone())
    })
    .listen(listener)?
    .run();
    Ok(server)
}
