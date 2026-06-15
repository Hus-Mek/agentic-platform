use thiserror::Error;

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Authentication error: {0}")]
    Auth(String),
    #[error("Authorization error: {0}")]
    Forbidden(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("LLM provider error: {0}")]
    LlmProvider(String),
    #[error("Tool execution error: {0}")]
    ToolExecution(String),
    #[error("Guard policy violation: {0}")]
    GuardViolation(String),
    #[error("Memory error: {0}")]
    Memory(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, AgentError>;
