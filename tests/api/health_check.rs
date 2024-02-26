use crate::helpers::spawn_app;

#[tokio::test]
async fn health_check_works() {
    // Launch our application as a background task
    let app = spawn_app().await;

    // Create a client to send request to our application
    let client = reqwest::Client::new();

    // Use the returned URL from our application to create a GET request
    let response = client
        .get(format!("{}/health_check", &app.address))
        .send()
        .await
        .expect("Failed to execute request.");

    // Assert that the response is a success
    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
