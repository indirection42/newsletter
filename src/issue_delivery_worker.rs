use crate::config::Settings;
use crate::domain::SubscriberEmail;
use crate::email_client::EmailClient;
use crate::startup::get_conn_pool;
use sqlx::{Executor, PgPool, Postgres, Transaction};
use std::time::Duration;
use uuid::Uuid;
pub async fn run_worker_until_stopped(config: Settings) -> Result<(), anyhow::Error> {
    let conn_pool = get_conn_pool(&config.database);
    let email_client = config.email_client.client();
    worker_loop(conn_pool, email_client).await
}
async fn worker_loop(pool: PgPool, email_client: EmailClient) -> Result<(), anyhow::Error> {
    loop {
        match try_execute_task(&pool, &email_client).await {
            Ok(ExecutionOutcome::TaskCompleted) => {}
            Ok(ExecutionOutcome::EmptyQueue) => {
                tokio::time::sleep(Duration::from_secs(10)).await;
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
pub enum ExecutionOutcome {
    TaskCompleted,
    EmptyQueue,
}
pub async fn try_execute_task(
    pool: &PgPool,
    email_client: &EmailClient,
) -> Result<ExecutionOutcome, anyhow::Error> {
    if let Some((tx, issue_id, email)) = dequeue_task(pool).await? {
        match email.clone().parse::<SubscriberEmail>() {
            Ok(email) => {
                let issue = get_issue(pool, issue_id).await?;
                if let Err(e) = email_client
                    .send_email(
                        &email,
                        &issue.title,
                        &issue.text_content,
                        &issue.html_content,
                    )
                    .await
                {
                    tracing::error!(
                        error.cause_chain =?e,
                        error.message = %e,
                        "Failed to send email to a confirmed subscriber. Skipping."
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    error.cause_chain =?e,
                    error.message = %e,
                    "Skipping a confirmed subscriber. Their stored contact details are invalid.");
            }
        }
        delete_task(tx, issue_id, &email).await?;
        Ok(ExecutionOutcome::TaskCompleted)
    } else {
        Ok(ExecutionOutcome::EmptyQueue)
    }
}

async fn dequeue_task(
    pool: &PgPool,
) -> Result<Option<(Transaction<'static, Postgres>, Uuid, String)>, anyhow::Error> {
    let mut tx = pool.begin().await?;
    if let Some(row) = sqlx::query!(
        r#"
        SELECT newsletter_issue_id, subscriber_email
        FROM issue_delivery_queue
        FOR UPDATE
        SKIP LOCKED
        LIMIT 1
        "#,
    )
    .fetch_optional(&mut *tx)
    .await?
    {
        Ok(Some((tx, row.newsletter_issue_id, row.subscriber_email)))
    } else {
        Ok(None)
    }
}

async fn delete_task(
    mut tx: Transaction<'static, Postgres>,
    issue_id: Uuid,
    email: &str,
) -> Result<(), anyhow::Error> {
    let query = sqlx::query!(
        r#"
        DELETE FROM issue_delivery_queue
        WHERE newsletter_issue_id = $1 AND subscriber_email = $2
        "#,
        issue_id,
        email
    );
    tx.execute(query).await?;
    tx.commit().await?;
    Ok(())
}

struct NewsletterIssue {
    title: String,
    text_content: String,
    html_content: String,
}

#[tracing::instrument(skip_all)]
async fn get_issue(pool: &PgPool, issue_id: Uuid) -> Result<NewsletterIssue, anyhow::Error> {
    let issue = sqlx::query_as!(
        NewsletterIssue,
        r#"
        SELECT title, text_content, html_content
        FROM newsletter_issues
        WHERE newsletter_issue_id = $1
        "#,
        issue_id
    )
    .fetch_one(pool)
    .await?;
    Ok(issue)
}
