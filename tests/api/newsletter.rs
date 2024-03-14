use crate::helpers::assert_is_redirect_to;
use crate::helpers::spawn_app;
use crate::helpers::ConfirmationLinks;
use crate::helpers::TestApp;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

    let request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content" : "Newsletter body as plain text",
        "html_content" : "<p>Newsletter body as HTML</p>"
    });

    let response = app.post_newsletters(request_body).await;

    assert_is_redirect_to(&response, "/login");
}
#[tokio::test]
async fn newsletter_returns_400_for_invalid_data() {
    let app = spawn_app().await;

    app.test_user.login(&app).await;

    let test_cases = vec![
        (serde_json::json!({}), "missing content and title"),
        (serde_json::json!({"title": "a title"}), "missing content"),
        (
            serde_json::json!(
                    {
            "text_content" : "Newsletter body as plain text",
            "html_content" : "<p>Newsletter body as HTML</p>"
                    }
                ),
            "missing title",
        ),
        (
            serde_json::json!({"title": "a title",
                    "html_content" : "<p>Newsletter body as HTML</p>"
            }),
            "missing text",
        ),
        (
            serde_json::json!({"title": "a title",  "text_content": "plain text" }),
            "missing html",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(invalid_body).await;
        assert_eq!(
            response.status().as_u16(),
            400,
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
#[tokio::test]
async fn newsletter_are_not_delivered_to_unconfirmed_subscriber() {
    let app = spawn_app().await;

    create_unconfirmed_subscriber(&app).await;

    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        // Assert no request is fired
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content" : "Newsletter body as plain text",
        "html_content" : "<p>Newsletter body as HTML</p>"
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscriber() {
    let app = spawn_app().await;

    create_confirmed_subscriber(&app).await;

    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        // Assert one request is fired
        .expect(1)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content" : "Newsletter body as plain text",
        "html_content" : "<p>Newsletter body as HTML</p>"
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=Hello%20Kitty&email=hello_kitty%40gmail.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .expect("Failed to create subscriber.");

    let email_request = &app.email_server.received_requests().await.unwrap()[0];

    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let links = create_unconfirmed_subscriber(app).await;
    reqwest::get(links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
