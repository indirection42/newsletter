use crate::session_state::TypedSession;
use crate::utils::e500;
use actix_web::http::header::LOCATION;
use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;

pub async fn logout(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user_id().map_err(e500)?.is_some() {
        session.logout();
        FlashMessage::info("You have successfully logged out").send();
    }
    Ok(HttpResponse::SeeOther()
        .insert_header((LOCATION, "/login"))
        .finish())
}
