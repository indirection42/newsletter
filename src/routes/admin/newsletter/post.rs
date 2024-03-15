use crate::authentication::UserId;
use crate::utils::see_other;
use crate::{domain::SubscriberEmail, email_client::EmailClient};
use actix_web::{
    http::{self, header::HeaderValue},
    web, HttpResponse, ResponseError,
};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use reqwest::header;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::warn;

#[derive(Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
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

#[tracing::instrument(name = "Publishing a newsletter.", skip(pool, email_client, form, user_id), fields(user_id = %*user_id))]
pub async fn publish_newsletter(
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, PublishError> {
    let subscribers = get_confirmed_subscribers(&pool)
        .await
        .context("Failed to get confirmed subscribers")?;

    for subscriber in subscribers {
        email_client
            .send_email(
                &subscriber.email,
                &form.title,
                &form.html_content,
                &form.text_content,
            )
            .await
            .with_context(|| {
                format!("Failed to send newsletter issue to {:?}", subscriber.email)
            })?;
    }
    FlashMessage::info("The newsletter issue has been published!").send();

    Ok(see_other("/admin/newsletters"))
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
