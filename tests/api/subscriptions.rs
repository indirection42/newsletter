use crate::helpers::spawn_app;
#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;

    let body = "name=Hello%20Kitty&email=hello_kitty%40gmail.com";

    let response = app.post_subscriptions(body.into()).await;

    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");
    assert_eq!(saved.name, "Hello Kitty");
    assert_eq!(saved.email, "hello_kitty@gmail.com");
}

#[tokio::test]
async fn subscribe_returns_a_400_for_invalid_form_data() {
    let app = spawn_app().await;

    let test_cases = vec![
        ("name=Hello%20Kitty", "missing the email"),
        ("email=hello_kitty%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
        (
            "name=DROP TABLE subscriptions;&email=hello_kitty%40gmail.com",
            "name contains SQL",
        ),
        ("name=Hello%20Kitty&email=notanemail", "invalid email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(invalid_body.into()).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}
