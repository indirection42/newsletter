use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::http::header::ContentType;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use reqwest::header::LOCATION;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn admin_dashboard(
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &pool).await.map_err(e500)?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
        <!DOCTYPE html>
        <html lang="en">
        <head>
            <meta http-equiv="Content-Type" content="text/html; charset=utf-8">
            <title>Admin Dashboard</title>
        </head>
        <body>
            <h1>Welcome {username}</h1>
            <p>Available actions:</p>
            <ol>
                <li><a href="/admin/password">Change password</a></li>
                <li>
                    <form name="logoutForm" action="/admin/logout" method="post">
                        <input type="submit" value="Logout">
                    </form>
                </li>
            </ol>
        </body>
        </html>
        "#
        )))
}

#[tracing::instrument(skip(pool))]
pub async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!("SELECT username FROM users WHERE user_id = $1", user_id)
        .fetch_one(pool)
        .await
        .context("Failed to perform a query to fetch username from the database.")?;
    Ok(row.username)
}
