use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::PgPool;
use tracing::error;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[tracing::instrument(name = "Confirming a pending subscriber.", skip(pool, parameters))]
pub async fn confirm(pool: web::Data<PgPool>, parameters: web::Query<Parameters>) -> HttpResponse {
    let subscription_token = &parameters.subscription_token;

    let subscriber_id = match get_subscriber_id_from_token(&pool, subscription_token).await {
        Ok(subscriber_id) => match subscriber_id {
            Some(subscriber_id) => subscriber_id,
            None => return HttpResponse::Unauthorized().finish(),
        },
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    if confirm_subscriber(&pool, subscriber_id).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }
    HttpResponse::Ok().finish()
}

#[tracing::instrument(name = "Mark subcriber as confirmed.", skip(pool, subscriber_id))]
pub async fn confirm_subscriber(pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions
        SET status = 'confirmed'
        WHERE id = $1
        "#,
        subscriber_id
    )
    .execute(pool)
    .await
    .map_err(|e| {
        error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(())
}

#[tracing::instrument(name = "Get subscriber_id from token", skip(pool, subscription_token))]
pub async fn get_subscriber_id_from_token(
    pool: &PgPool,
    subscription_token: &str,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1
        "#,
        subscription_token,
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        error!("Failed to execute query: {:?}", e);
        e
    })?;
    Ok(result.map(|r| r.subscriber_id))
}
