use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;
use sqlx::{
    postgres::{PgConnectOptions, PgSslMode},
    ConnectOptions,
};
#[derive(Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub email_client: EmailClientSettings,
}

#[derive(Deserialize, Clone)]
pub struct ApplicationSettings {
    pub host: String,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub base_url: String,
    pub hmac_secret: Secret<String>,
}

#[derive(Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
}

pub fn get_config() -> Result<Settings, config::ConfigError> {
    let base_path = std::env::current_dir().expect("Failed to determine the current directory.");
    let config_dir = base_path.join("config");
    let env = std::env::var("APP_ENV").unwrap_or_else(|_| "local".into());
    let env = match env.to_lowercase().as_str() {
        "local" | "dev" | "development" => "local",
        "prod" | "production" => "production",
        other => panic!("Unknown environment: {:?}", other),
    };
    let settings = config::Config::builder()
        .add_source(config::File::from(config_dir.join("base")).required(true))
        .add_source(config::File::from(config_dir.join(env)).required(true))
        .add_source(
            config::Environment::with_prefix("app")
                .prefix_separator("_")
                .separator("__"),
        );
    settings.build()?.try_deserialize()
}

impl DatabaseSettings {
    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_mode = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };
        PgConnectOptions::new()
            .host(&self.host)
            .username(&self.username)
            .password(self.password.expose_secret())
            .port(self.port)
            .ssl_mode(ssl_mode)
    }
    pub fn with_db(&self) -> PgConnectOptions {
        let options = self.without_db().database(&self.database_name);
        options.log_statements(tracing::log::LevelFilter::Trace)
    }
}

#[derive(Deserialize, Clone)]
pub struct EmailClientSettings {
    pub base_url: String,
    pub sender_email: String,
    pub authorization_token: Secret<String>,
    pub timeout_milliseconds: u64,
}

impl EmailClientSettings {
    pub fn client(self) -> EmailClient {
        let timeout = self.timeout();

        let sender_email = self.sender().expect("Invalid sender email address.");

        EmailClient::new(
            self.base_url,
            sender_email,
            self.authorization_token,
            timeout,
        )
    }
    pub fn sender(&self) -> Result<SubscriberEmail, String> {
        self.sender_email.parse()
    }
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_millis(self.timeout_milliseconds)
    }
}
