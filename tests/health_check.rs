use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::io;
use std::net::TcpListener;
use zero2prod::config::get_config;
use zero2prod::startup::run;
use zero2prod::telemetry::{get_subsciber, init_subscriber};

static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subsciber("test".into(), "debug".into(), io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subsciber("test".into(), "debug".into(), io::sink);
        init_subscriber(subscriber);
    };
});
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

#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data() {
    let app = spawn_app().await;
    // set up client
    let client = reqwest::Client::new();

    let body = "name=Hello%20Kitty&email=hello_kitty%40gmail.com";
    let response = client
        .post(format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

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
    let client = reqwest::Client::new();

    let test_cases = vec![
        ("name=Hello%20Kitty", "missing the email"),
        ("email=hello_kitty%40gmail.com", "missing the name"),
        ("", "missing both name and email"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = client
            .post(format!("{}/subscriptions", &app.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(invalid_body)
            .send()
            .await
            .expect("Failed to execute request.");
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            error_message
        );
    }
}

struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}
async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{}", port);
    let conn_pool = configure_database().await;
    let server = run(listener, conn_pool.clone()).expect("Failed to bind address");
    tokio::spawn(server);
    TestApp {
        address,
        db_pool: conn_pool,
    }
}

async fn configure_database() -> PgPool {
    let mut config = get_config().expect("Failed to read configuration.");
    let mut conn = PgConnection::connect_with(&config.database.without_db())
        .await
        .expect("Failed to connect to Postgres.");
    // Generate a randomized database name to ensure that our tests don't conflict between runs
    config.database.database_name = uuid::Uuid::new_v4().to_string();
    conn.execute(format!(r#"CREATE DATABASE "{}";"#, &config.database.database_name).as_str())
        .await
        .expect("Failed to create database.");
    let conn_pool = PgPool::connect_with(config.database.with_db())
        .await
        .expect("Failed to connect to Postgres.");
    sqlx::migrate!("./migrations")
        .run(&conn_pool)
        .await
        .expect("Failed to run migrations.");
    conn_pool
}
