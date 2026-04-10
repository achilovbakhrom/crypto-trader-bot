use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    /// Wraps any error returned by sqlx (query, connection, pool, …).
    #[error("database error: {0}")]
    SqlxError(#[from] sqlx::Error),

    /// Wraps errors that occur while running embedded migrations.
    #[error("migration error: {0}")]
    MigrationError(#[from] sqlx::migrate::MigrateError),

    /// Returned when a requested row does not exist.
    #[error("record not found")]
    NotFound,

    /// Returned when a value cannot be decoded into the expected Rust type.
    #[error("type conversion error: {0}")]
    ConversionError(String),
}
