use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),

    #[error("Environment variable missing: {0}")]
    EnvVar(String),

    #[error("Parse error: {0}")]
    Parse(String),
}
