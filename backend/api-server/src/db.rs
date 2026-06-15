use sqlx::postgres::PgPool;

pub async fn create_db_pool(database_url: &str) -> PgPool {
    PgPool::connect(database_url).await.unwrap()
}

pub async fn run_migrations(pool: &PgPool) {
    sqlx::migrate!("../migrations").run(pool).await.unwrap();
}
