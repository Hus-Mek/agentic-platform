use sqlx::postgres::{PgPool, PgPoolOptions};

pub async fn create_db_pool(database_url: &str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed to create database pool")
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("../migrations").run(pool).await.unwrap();
}
