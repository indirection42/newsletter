use crate::authentication::UserId;
use crate::idempotency::{get_saved_response, save_response, IdempotencyKey};
use crate::utils::{e400, e500, see_other};
use crate::{domain::SubscriberEmail, email_client::EmailClient};
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use anyhow::Context;
use serde::Deserialize;
use sqlx::PgPool;
use tracing::warn;

#[derive(Deserialize)]
pub struct FormData {
    title: String,
    html_content: String,
    text_content: String,
    idempotency_key: String,
}

#[tracing::instrument(name = "Publishing a newsletter.", skip(pool, email_client, form, user_id), fields(user_id = %*user_id))]
pub async fn publish_newsletter(
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    form: web::Form<FormData>,
    user_id: web::ReqData<UserId>,
) -> Result<HttpResponse, actix_web::Error> {
    let FormData {
        title,
        html_content,
        text_content,
        idempotency_key,
    } = form.into_inner();

    let idempotency_key: IdempotencyKey = idempotency_key.try_into().map_err(e400)?;

    if let Some(saved_response) = get_saved_response(&pool, &idempotency_key, **user_id)
        .await
        .map_err(e500)?
    {
        FlashMessage::info("The newsletter issue has been published!").send();
        return Ok(saved_response);
    }
    let subscribers = get_confirmed_subscribers(&pool).await.map_err(e500)?;

    for subscriber in subscribers {
        email_client
            .send_email(&subscriber.email, &title, &html_content, &text_content)
            .await
            .with_context(|| format!("Failed to send newsletter issue to {:?}", subscriber.email))
            .map_err(e500)?;
    }
    FlashMessage::info("The newsletter issue has been published!").send();
    let response = see_other("/admin/newsletters");
    let response = save_response(&pool, &idempotency_key, **user_id, response)
        .await
        .map_err(e500)?;
    Ok(response)
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
