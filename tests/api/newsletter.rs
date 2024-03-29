use crate::helpers::assert_is_redirect_to;
use crate::helpers::spawn_app;
use crate::helpers::ConfirmationLinks;
use crate::helpers::TestApp;
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use std::time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn concurrent_form_submission_is_handled_gracefully() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(2)))
        // For idempotency, we expect only one email request to be fired
        .expect(1)
        .mount(&app.email_server)
        .await;

    let request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content" : "Newsletter body as plain text",
        "html_content" : "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response1 = app.post_newsletters(&request_body);
    let response2 = app.post_newsletters(&request_body);
    let (response1, response2) = tokio::join!(response1, response2);

    assert_eq!(response1.status(), response2.status());
    assert_eq!(
        response1.text().await.unwrap(),
        response2.text().await.unwrap()
    );
    app.dispatch_all_pending_emails().await;
}
#[tokio::test]
async fn newsletters_creation_is_idempotent() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    app.test_user.login(&app).await;

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        // For idempotency, we expect only one email request to be fired
        .expect(1)
        .mount(&app.email_server)
        .await;

    let request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content" : "Newsletter body as plain text",
        "html_content" : "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });
    let response = app.post_newsletters(&request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletters_html().await;

    assert!(
        html_page.contains("The newsletter issue has been accepted - emails will go out shortly.")
    );

    let response = app.post_newsletters(&request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletters_html().await;
    assert!(
        html_page.contains("The newsletter issue has been accepted - emails will go out shortly.")
    );
    app.dispatch_all_pending_emails().await;
}
#[tokio::test]
async fn requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

    let request_body = serde_json::json!({
        "title": "Newsletter title",
        "text_content" : "Newsletter body as plain text",
        "html_content" : "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&request_body).await;

    assert_is_redirect_to(&response, "/login");
}
#[tokio::test]
async fn newsletter_returns_400_for_invalid_data() {
    let app = spawn_app().await;

    app.test_user.login(&app).await;

    let test_cases = vec![
        (serde_json::json!({}), "missing content and title"),
        (
            serde_json::json!({"title": "a title",        "idempotency_key": uuid::Uuid::new_v4().to_string()}),
            "missing content",
        ),
        (
            serde_json::json!({
                "text_content" : "Newsletter body as plain text",
                "html_content" : "<p>Newsletter body as HTML</p>",
                "idempotency_key": uuid::Uuid::new_v4().to_string()
                }
            ),
            "missing title",
        ),
        (
            serde_json::json!(
                {"title": "a title",
                        "html_content" : "<p>Newsletter body as HTML</p>",
            "idempotency_key": uuid::Uuid::new_v4().to_string()
                }),
            "missing text",
        ),
        (
            serde_json::json!({"title": "a title",  "text_content": "plain text",        "idempotency_key": uuid::Uuid::new_v4().to_string() }),
            "missing html",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_newsletters(&invalid_body).await;
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
        "html_content" : "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletters");
    app.dispatch_all_pending_emails().await;
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
        "html_content" : "<p>Newsletter body as HTML</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string()
    });

    let response = app.post_newsletters(&newsletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletters_html().await;
    assert!(
        html_page.contains("The newsletter issue has been accepted - emails will go out shortly.")
    );
    app.dispatch_all_pending_emails().await;
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(serde_json::json!({
        "name": &name,
        "email": &email
    }))
    .unwrap();

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body)
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
