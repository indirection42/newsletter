use actix_web::http::header::LOCATION;
use actix_web::{web, HttpResponse};
use actix_web_flash_messages::FlashMessage;
use secrecy::{ExposeSecret, Secret};
use serde::Deserialize;
use sqlx::PgPool;

use crate::authentication::{validate_credentials, AuthError, Credentials};
use crate::routes::admin::get_username;
use crate::session_state::TypedSession;
use crate::utils::e500;

#[derive(Deserialize)]
pub struct FormData {
    current_password: Secret<String>,
    new_password: Secret<String>,
    new_password_check: Secret<String>,
}

pub async fn change_password(
    form: web::Form<FormData>,
    session: TypedSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_id = session.get_user_id().map_err(e500)?;
    if user_id.is_none() {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/login"))
            .finish());
    };
    let user_id = user_id.unwrap();
    // Check if the new passwords match
    if form.new_password.expose_secret() != form.new_password_check.expose_secret() {
        FlashMessage::error("New passwords do not match").send();
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/admin/password"))
            .finish());
    }
    // Check if new password is long enough
    if form.new_password.expose_secret().len() < 12 {
        FlashMessage::error("The new password is too short").send();
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, "/admin/password"))
            .finish());
    }
    // Check if the current password is correct
    let username = get_username(user_id, &pool).await.map_err(e500)?;
    let credentials = Credentials {
        username,
        password: form.0.current_password,
    };
    if let Err(e) = validate_credentials(credentials, &pool).await {
        return match e {
            AuthError::InvalidCredentials(_) => {
                FlashMessage::error("The current password is incorrect").send();
                Ok(HttpResponse::SeeOther()
                    .insert_header((LOCATION, "/admin/password"))
                    .finish())
            }
            AuthError::UnexpectedError(_) => Err(e500(e)),
        };
    }
    // Change password
    crate::authentication::change_password(user_id, form.0.new_password, &pool)
        .await
        .map_err(e500)?;
    FlashMessage::error("Your password has been changed").send();

    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/admin/password"))
        .finish())
}
