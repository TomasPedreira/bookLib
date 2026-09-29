mod catalog;
mod library;

use axum::{
    extract::DefaultBodyLimit,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;
use sqlx::{
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
    SqlitePool,
};
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct AppState {
    db: SqlitePool,
    http: reqwest::Client,
    catalog_gate: Arc<tokio::sync::Mutex<Instant>>,
    catalog_cache: Arc<tokio::sync::Mutex<HashMap<String, (Instant, serde_json::Value)>>>,
}

#[derive(Debug)]
struct AppError(StatusCode, String);
type ApiResult<T> = Result<Json<T>, AppError>;

impl axum::response::IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("booklib=info")
        .init();
    let project_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let db_path = std::env::var("BOOKLIB_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| project_dir.join("booklib.sqlite"));
    let options = SqliteConnectOptions::new()
        .filename(db_path)
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
        .user_agent("BookLib/0.1 (personal reading library)")
        .timeout(Duration::from_secs(8))
        .build()?;
    let state = AppState {
        db,
        http,
        catalog_gate: Arc::new(tokio::sync::Mutex::new(
            Instant::now() - Duration::from_secs(1),
        )),
        catalog_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
    };
    let app = Router::new()
        .route(
            "/api/books",
            get(library::list_books).post(library::create_book),
        )
        .route(
            "/api/books/:id",
            get(library::get_book)
                .put(library::update_book)
                .delete(library::delete_book),
        )
        .route("/api/books/:id/readings", post(library::start_reading))
        .route(
            "/api/readings/:id",
            axum::routing::patch(library::change_reading),
        )
        .route("/api/readings/:id/progress", post(library::add_progress))
        .route(
            "/api/progress/:id",
            axum::routing::put(library::edit_progress).delete(library::delete_progress),
        )
        .route("/api/stats", get(library::stats))
        .route("/api/export", get(library::export_data))
        .route("/api/import", post(library::import_data))
        .route("/api/catalog", get(catalog::search))
        .route("/api/catalog/isbn/:isbn", get(catalog::lookup_isbn))
        .route("/api/recommendations", get(catalog::recommendations))
        .route("/api/catalog/works/:id", get(catalog::work_details))
        .route("/api/catalog/works/:id/editions", get(catalog::editions))
        .route("/api/catalog/works/:id/genres", get(catalog::work_genres))
        .fallback_service(
            ServeDir::new(project_dir.join("web")).append_index_html_on_directories(true),
        )
        .layer(DefaultBodyLimit::max(30 * 1024 * 1024))
        .with_state(state);
    let addr: SocketAddr = std::env::var("BOOKLIB_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:3000".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("BookLib em http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
