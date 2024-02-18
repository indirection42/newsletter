use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
}

#[derive(Deserialize)]
pub struct ApplicationSettings {
    pub host: String,
    pub port: u16,
}

#[derive(Deserialize)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: Secret<String>,
    pub port: u16,
    pub host: String,
    pub database_name: String,
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
        .add_source(config::File::from(config_dir.join(env)).required(true));
    settings.build()?.try_deserialize()
}

impl DatabaseSettings {
    pub fn connection_string(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}/{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port,
            self.database_name
        ))
    }
    pub fn connection_string_without_db(&self) -> Secret<String> {
        Secret::new(format!(
            "postgres://{}:{}@{}:{}",
            self.username,
            self.password.expose_secret(),
            self.host,
            self.port
        ))
    }
}
