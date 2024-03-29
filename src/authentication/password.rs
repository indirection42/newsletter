use anyhow::Context;
use argon2::password_hash::SaltString;
use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use secrecy::{ExposeSecret, Secret};
use sqlx::PgPool;

#[derive(thiserror::Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials.")]
    InvalidCredentials(#[source] anyhow::Error),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

#[derive(Debug)]
pub struct Credentials {
    pub username: String,
    pub password: Secret<String>,
}

#[tracing::instrument(name = "Validating credentials.", skip(credentials, pool))]
pub async fn validate_credentials(
    credentials: Credentials,
    pool: &PgPool,
) -> Result<uuid::Uuid, AuthError> {
    let (user_id, expected_password_hash) =
        match get_stored_credentials(&credentials.username, pool).await? {
            Some((user_id, expected_password_hash)) => (Some(user_id), expected_password_hash),
            None => (
                None,
                // Dummy password hash to make the function time-constant
                Secret::new(
                    "$argon2id$v=19$m=15000,t=2,p=1$gZiV/M1gPc22ElAH/Jh1Hw$CWOrkoo7oJBQ/iyh7uJ0LO2aLEfrHwTWllSAxT0zRno"
                        .to_string(),
                ),
            ),
        };

    match tokio::task::spawn_blocking(move || {
        verify_password_hash(expected_password_hash, credentials.password)
    })
    .await
    .context("Failed to spawn blocking task.")?
    {
        Ok(_) => Ok(()),
        Err(e) => match e {
            AuthError::InvalidCredentials(_) => {
                if user_id.is_none() {
                    Ok(())
                } else {
                    Err(e)
                }
            }
            _ => Err(e),
        },
    }?;

    user_id
        .ok_or_else(|| anyhow::anyhow!("Unknown username."))
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Fetching stored credentials.", skip(username, pool))]
async fn get_stored_credentials(
    username: &str,
    pool: &PgPool,
) -> Result<Option<(uuid::Uuid, Secret<String>)>, anyhow::Error> {
    let record = sqlx::query!(
        "SELECT user_id, password_hash FROM users WHERE username = $1",
        username
    )
    .fetch_optional(pool)
    .await
    .context("Failed to perform a query to validate auth credentials.")?
    .map(|record| (record.user_id, Secret::new(record.password_hash)));

    Ok(record)
}

#[tracing::instrument(
    name = "Verifying password hash.",
    skip(expected_password_hash, password_candidate)
)]
fn verify_password_hash(
    expected_password_hash: Secret<String>,
    password_candidate: Secret<String>,
) -> Result<(), AuthError> {
    let expected_password_hash = PasswordHash::new(expected_password_hash.expose_secret())
        .context("Failed to parse hash in PHC String format.")?;
    Argon2::default()
        .verify_password(
            password_candidate.expose_secret().as_bytes(),
            &expected_password_hash,
        )
        .context("Invalid password.")
        .map_err(AuthError::InvalidCredentials)
}

#[tracing::instrument(name = "Change password", skip(password, pool))]
pub async fn change_password(
    user_id: uuid::Uuid,
    password: Secret<String>,
    pool: &PgPool,
) -> Result<(), anyhow::Error> {
    let password_hash = tokio::task::spawn_blocking(move || compute_password_hash(password))
        .await?
        .context("Failed to hash password")?;
    sqlx::query!(
        "UPDATE users SET password_hash = $1 WHERE user_id = $2",
        password_hash.expose_secret(),
        user_id
    )
    .execute(pool)
    .await
    .context("Failed to change user's password in the database")?;
    Ok(())
}

fn compute_password_hash(password: Secret<String>) -> Result<Secret<String>, anyhow::Error> {
    let salt = SaltString::generate(&mut rand::thread_rng());
    let password_hash = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(15000, 2, 1, None).unwrap(),
    )
    .hash_password(password.expose_secret().as_bytes(), &salt)?
    .to_string();
    Ok(Secret::new(password_hash))
}
