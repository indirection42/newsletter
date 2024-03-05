use crate::{domain::SubscriberEmail, email_client::EmailClient};
use actix_web::{
    http::{
        self,
        header::{HeaderMap, HeaderValue},
    },
    web, HttpRequest, HttpResponse, ResponseError,
};
use anyhow::Context;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use base64::prelude::*;
use reqwest::header;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::warn;

#[derive(Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("Authentication failed.")]
    AuthError(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for PublishError {
    fn error_response(&self) -> HttpResponse {
        match self {
            PublishError::UnexpectedError(_) => {
                HttpResponse::new(http::StatusCode::INTERNAL_SERVER_ERROR)
            }
            PublishError::AuthError(_) => {
                let mut response = HttpResponse::new(http::StatusCode::UNAUTHORIZED);
                let header_value = HeaderValue::from_str(r#"Basic realm="publish""#).unwrap();
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, header_value);
                response
            }
        }
    }
}

#[tracing::instrument(name = "Publishing a newsletter.", skip(pool, email_client, body))]
pub async fn publish_newsletter(
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    body: web::Json<BodyData>,
    request: HttpRequest,
) -> Result<HttpResponse, PublishError> {
    let credentials = basic_authentication(request.headers()).map_err(PublishError::AuthError)?;

    let _user_id = validate_credentials(credentials, &pool).await?;

    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .context("Failed to get confirmed subscribers")?;

    for subscriber in subscribers {
        email_client
            .send_email(
                &subscriber.email,
                &body.title,
                &body.content.html,
                &body.content.text,
            )
            .await
            .with_context(|| {
                format!("Failed to send newsletter issue to {:?}", subscriber.email)
            })?;
    }

    Ok(HttpResponse::Ok().finish())
}
#[derive(Debug)]
struct Credentials {
    username: String,
    password: Secret<String>,
}
fn basic_authentication(headers: &HeaderMap) -> Result<Credentials, anyhow::Error> {
    let header_value = headers
        .get("Authorization")
        .context("The 'Authorization' header is missing.")?
        .to_str()
        .context("The 'Authorization' header was not a valid string.")?;
    let base64encoded_segment = header_value
        .strip_prefix("Basic ")
        .context("The authorization scheme was not 'Basic {BASE64_ENCODED}'. ")?;
    let decoded_bytes = BASE64_STANDARD
        .decode(base64encoded_segment.as_bytes())
        .context("Failed to base64-decode 'Basic' credentials")?;
    let decoded_credentials = String::from_utf8(decoded_bytes)
        .context("The 'Basic' credentials were not valid UTF-8.")?;

    let mut credentials = decoded_credentials.splitn(2, ':');

    let username = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("The username was not present in the 'Basic' credentials."))?
        .to_string();

    let password = credentials
        .next()
        .ok_or_else(|| anyhow::anyhow!("The password was not present in the 'Basic' credentials."))?
        .to_string();
    Ok(Credentials {
        username,
        password: Secret::new(password),
    })
}

#[tracing::instrument(name = "Validating credentials.", skip(credentials, pool))]
async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, PublishError> {
    let (user_id, expected_password_hash) = get_stored_credentials(&credentials.username, pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(PublishError::AuthError)?;

    tokio::task::spawn_blocking(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")?
    .context("Invalid password.")
    .map_err(PublishError::AuthError)?;
    Ok(user_id)
}

#[tracing::instrument(name = "Fetching stored credentials.", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let record = sqlx::query!(
        "SELECT user_id, password_hash FROM users WHERE username = $1",
        username
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?
    .map(|record| (record.user_id, Secret::new(record.password_hash)));

    Ok(record)
}

#[tracing::instrument(
    name = "Verifying password hash.",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), PublishError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC String format.")?;
    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(PublishError::AuthError)
}

struct ConfirmedSubscriber {
    email: SubscriberEmail,
}

#[tracing::instrument(name = "Fetching confirmed subscribers.", skip(pool))]
async fn get_confirmed_subscribers(pool: &PgPool) -> Result<Vec<ConfirmedSubscriber>, sqlx::Error> {
    let confirmed_subscribers =
        sqlx::query!(r#"SELECT email FROM subscriptions WHERE status = 'confirmed'"#,)
            .fetch_all(pool)
            .await?
            .into_iter()
            .filter_map(|r| match r.email.parse() {
                Ok(email) => Some(ConfirmedSubscriber { email }),
                Err(e) => {
                    warn!(
                        "A confirmed subscriber is using an invalid email address.\n{}.",
                        e
                    );
                    None
                }
            })
            .collect();

    Ok(confirmed_subscribers)
}
