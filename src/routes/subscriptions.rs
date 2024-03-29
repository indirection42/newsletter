use crate::{
    domain::{NewSubscriber, SubscriberEmail, SubscriberName},
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
};
use actix_web::{http, web, HttpResponse, ResponseError};
use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use serde::Deserialize;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use tracing::error;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> http::StatusCode {
        match self {
            SubscribeError::ValidationError(_) => http::StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryFrom<FormData> for NewSubscriber {
    type Error = String;
    fn try_from(form: FormData) -> Result<Self, Self::Error> {
        let name = form.name.parse::<SubscriberName>()?;
        let email = form.email.parse::<SubscriberEmail>()?;
        Ok(NewSubscriber { email, name })
    }
}
#[tracing::instrument(
    name = "Adding a new subscriber.",
    skip(form, pool, email_client, base_url),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
) -> Result<HttpResponse, SubscribeError> {
    let new_subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;

    let mut transaction = pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool")?;
    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .context("Failed to insert new subscriber in the database")?;
    let subscription_token = gen_subscription_token();
    store_token(&mut transaction, subscriber_id, &subscription_token)
        .await
        .context("Failed to store the confirmation token in the database")?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber")?;
    send_confirmation_email(
        &email_client,
        new_subscriber,
        &base_url.0,
        &subscription_token,
    )
    .await
    .context("Failed to send a confirmation email.")?;
    Ok(HttpResponse::Ok().finish())
}
#[tracing::instrument(
    name = "Saving new subscriber details in the database.",
    skip(transaction, new_subscriber)
)]
pub async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
    );
    transaction.execute(query).await?;
    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Store subscription_token in the database",
    skip(transaction, subscription_token)
)]
pub async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"
        INSERT INTO subscription_tokens (subscription_token, subscriber_id)
        VALUES ($1, $2)
        "#,
        subscription_token,
        subscriber_id
    );
    transaction.execute(query).await?;
    Ok(())
}

#[tracing::instrument(
    name = "Sending confirmation email to new subscriber.",
    skip(email_client, new_subscriber, base_url)
)]
pub async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    base_url: &str,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link =
        format!("{base_url}/subscriptions/confirm?subscription_token={subscription_token}");
    let subject = "Welcome!";
    let text_body = format!(
        "Welcome to our newsletter!\nVisit {} to confirm your subscription.",
        confirmation_link
    );
    let html_body = format!("Welcome to our newsletter!<br />\nClick <a href=\"{}\">here</a> to confirm your subscription.", confirmation_link);
    email_client
        .send_email(&new_subscriber.email, subject, &html_body, &text_body)
        .await
}

fn gen_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}
