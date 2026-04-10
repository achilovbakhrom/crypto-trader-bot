use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tracing::info;

use crate::error::StorageError;

/// A thin wrapper around a [`sqlx::PgPool`] that also knows how to run
/// embedded migrations.
pub struct Database {
    pool: PgPool,
}

impl Database {
    /// Connect to Postgres and return a ready-to-use [`Database`].
    ///
    /// # Arguments
    /// * `url` – full Postgres connection URL (e.g. `postgres://user:pass@host/db`)
    /// * `max_connections` – upper bound for the connection pool
    /// * `min_connections` – connections kept alive even when idle
    pub async fn connect(
        url: &str,
        max_connections: u32,
        min_connections: u32,
    ) -> Result<Self, StorageError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .connect(url)
            .await?;

        info!(max_connections, min_connections, "connected to Postgres");
        Ok(Self { pool })
    }

    /// Run all pending embedded SQL migrations from the `migrations/` folder.
    ///
    /// This is idempotent – already-applied migrations are skipped.
    pub async fn run_migrations(&self) -> Result<(), StorageError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        info!("database migrations applied");
        Ok(())
    }

    /// Return a reference to the underlying connection pool.
    ///
    /// Callers can pass `self.pool()` directly to the query functions in
    /// [`crate::queries`].
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
