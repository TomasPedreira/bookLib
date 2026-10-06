pub mod catalog;
mod catalog_cache;
pub mod library;

use reqwest::StatusCode;
use serde::Serialize;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct AppState {
    db: SqlitePool,
    http: reqwest::Client,
    catalog_gate: Arc<tokio::sync::Mutex<Instant>>,
    catalog_cache: Arc<catalog_cache::CatalogCache>,
}

#[derive(Clone, Debug)]
pub struct AppError(StatusCode, String);
pub type ServiceResult<T> = Result<T, AppError>;

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut error = serializer.serialize_struct("AppError", 2)?;
        error.serialize_field("code", &self.0.as_u16())?;
        error.serialize_field("message", &self.1)?;
        error.end()
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.1)
    }
}
impl std::error::Error for AppError {}
impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(%error, "database error");
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Erro na base de dados".into(),
        )
    }
}
fn bad(message: impl Into<String>) -> AppError {
    AppError(StatusCode::BAD_REQUEST, message.into())
}
fn missing() -> AppError {
    AppError(StatusCode::NOT_FOUND, "Registo não encontrado".into())
}

impl AppState {
    pub async fn open(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));
        let db = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;
        sqlx::migrate!("./migrations").run(&db).await?;
        let http = reqwest::Client::builder()
            .user_agent("BookLib/0.2 (personal reading library)")
            .timeout(Duration::from_secs(8))
            .build()?;
        Ok(Self {
            db,
            http,
            catalog_gate: Arc::new(tokio::sync::Mutex::new(
                Instant::now() - Duration::from_secs(1),
            )),
            catalog_cache: Arc::new(catalog_cache::CatalogCache::default()),
        })
    }
}
