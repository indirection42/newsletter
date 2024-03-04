use crate::{domain::SubscriberEmail, email_client::EmailClient};
use actix_web::{http, web, HttpResponse, ResponseError};
use anyhow::Context;
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
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for PublishError {
    fn status_code(&self) -> http::StatusCode {
        match self {
            PublishError::UnexpectedError(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[tracing::instrument(name = "Publishing a newsletter.", skip(pool, email_client, body))]
pub async fn publish_newsletter(
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    body: web::Json<BodyData>,
) -> Result<HttpResponse, PublishError> {
    let subscribers = get_confirmed_subscribers(pool.get_ref())
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
