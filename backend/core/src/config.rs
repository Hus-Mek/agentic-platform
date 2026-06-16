use std::env;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub llm_api_key: String,
    pub llm_base_url: String,
    pub embedding_base_url: String,
    pub qdrant_url: String,
    pub environment: String,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost:5432/agentic_platform".into()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "dev-secret-change-in-production".into()),
            llm_api_key: env::var("LLM_API_KEY")
                .unwrap_or_else(|_| env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-change-in-production".into())),
            llm_base_url: env::var("LLM_BASE_URL")
                .unwrap_or_else(|_| "https://openrouter.ai/api/v1".into()),
            embedding_base_url: env::var("EMBEDDING_BASE_URL")
                .unwrap_or_else(|_| env::var("LLM_BASE_URL").unwrap_or_else(|_| "https://openrouter.ai/api/v1".into())),
            qdrant_url: env::var("QDRANT_URL")
                .unwrap_or_else(|_| "http://localhost:6333".into()),
            environment: env::var("ENVIRONMENT")
                .unwrap_or_else(|_| "development".into()),
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".into())
                .parse()
                .expect("PORT must be a number"),
        }
    }
}
