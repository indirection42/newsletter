use actix_web::http::header::ContentType;
use actix_web::HttpResponse;
use actix_web_flash_messages::{IncomingFlashMessages, Level};
pub async fn publish_newsletter_form(
    flash_messages: IncomingFlashMessages,
) -> Result<HttpResponse, actix_web::Error> {
    let mut error_html = String::new();
    for m in flash_messages.iter().filter(|m| m.level() == Level::Error) {
        error_html.push_str(&format!(r#"<p style="color: red;">{}</p>"#, m.content()));
    }
    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta http-equiv="content-type" content="text/html; charset=utf-8">
    <title>Send a Newsletter Issue</title>
</head>
<body>
    {error_html}
    <form action="/admin/newsletters" method="post">
        <label>Newsletter Title:<br>
            <input
                type="text"
                placeholder="Enter newsletter title"
                name="title"
            >
        </label>
        <br>
        <label>Plain Text Content:<br>
            <textarea
                placeholder="Enter the content in plain text content"
                name="text_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <br>
        <label>HTML Content:<br>
            <textarea
                placeholder="Enter the content in HTML format"
                name="html_content"
                rows="20"
                cols="50"
            ></textarea>
        </label>
        <button type="submit">Send</button>
    </form>
    <p><a href="/admin/dashboard">&lt;- Back</a></p>
</body>
</html>"#
        )))
}
