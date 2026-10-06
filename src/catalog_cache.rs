use crate::{AppError, AppState, ServiceResult};
use reqwest::StatusCode;
use serde_json::Value;
use sqlx::{FromRow, SqlitePool};
use std::{
    collections::HashMap,
    sync::{Arc, Weak},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::Mutex;

const DAY: i64 = 86_400;
const RETRY_DELAY: i64 = 60;
const NOT_FOUND_TTL: i64 = 6 * 3_600;
const MEMORY_LIMIT: usize = 128;
const MEMORY_BYTES: usize = 8 * 1_024 * 1_024;
const DISK_LIMIT: i64 = 512;
const DISK_BYTES: i64 = 32 * 1_024 * 1_024;
const MAX_PAYLOAD_BYTES: usize = 2 * 1_024 * 1_024;

#[derive(Default)]
pub(crate) struct CatalogCache {
    memory: Mutex<HashMap<String, (Instant, Entry)>>,
    requests: Mutex<HashMap<String, Weak<Mutex<()>>>>,
}

#[derive(Clone)]
struct Entry {
    result: ServiceResult<Value>,
    fetched_at: i64,
    fresh_until: i64,
    stale_until: i64,
    retry_after: i64,
    bytes: usize,
}

#[derive(FromRow)]
struct StoredEntry {
    status: i64,
    payload: Option<String>,
    fetched_at: i64,
    fresh_until: i64,
    stale_until: i64,
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn key(url: &str, params: &[(&str, &str)]) -> String {
    let mut params = params.to_vec();
    params.sort_unstable();
    // Version the raw-response format independently of the app and its recommendations.
    format!("v1:{url}:{}", serde_json::to_string(&params).unwrap())
}

fn not_found() -> AppError {
    AppError(
        StatusCode::NOT_FOUND,
        "Edição não encontrada no catálogo".into(),
    )
}

impl CatalogCache {
    async fn request_lock(&self, key: &str) -> Arc<Mutex<()>> {
        let mut requests = self.requests.lock().await;
        requests.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = requests.get(key).and_then(Weak::upgrade) {
            return lock;
        }
        let lock = Arc::new(Mutex::new(()));
        requests.insert(key.to_owned(), Arc::downgrade(&lock));
        lock
    }

    async fn remember(&self, key: &str, entry: Entry) {
        let mut memory = self.memory.lock().await;
        let timestamp = now();
        memory.retain(|_, (_, entry)| entry.stale_until > timestamp);
        memory.insert(key.to_owned(), (Instant::now(), entry));
        while memory.len() > MEMORY_LIMIT
            || memory.values().map(|(_, entry)| entry.bytes).sum::<usize>() > MEMORY_BYTES
        {
            let oldest = memory
                .iter()
                .min_by_key(|(_, (at, _))| *at)
                .map(|(key, _)| key.clone())
                .unwrap();
            memory.remove(&oldest);
        }
    }

    async fn load(&self, db: &SqlitePool, key: &str) -> Option<Entry> {
        let timestamp = now();
        {
            let mut memory = self.memory.lock().await;
            if let Some((accessed, entry)) = memory.get_mut(key) {
                if entry.stale_until > timestamp {
                    *accessed = Instant::now();
                    return Some(entry.clone());
                }
            }
            memory.remove(key);
        }
        let stored = match sqlx::query_as::<_, StoredEntry>(
            "SELECT status, payload, fetched_at, fresh_until, stale_until
             FROM catalog_cache WHERE cache_key = ? AND stale_until > ?",
        )
        .bind(key)
        .bind(timestamp)
        .fetch_optional(db)
        .await
        {
            Ok(stored) => stored?,
            Err(error) => {
                tracing::warn!(%error, "catalog cache read failed");
                return None;
            }
        };
        let bytes = stored.payload.as_ref().map_or(0, String::len);
        let result = match stored.status {
            404 => Err(not_found()),
            200 if bytes <= MAX_PAYLOAD_BYTES => stored
                .payload
                .as_deref()
                .and_then(|payload| serde_json::from_str(payload).ok())
                .map(Ok)?,
            _ => return None,
        };
        let entry = Entry {
            result,
            fetched_at: stored.fetched_at,
            fresh_until: stored.fresh_until,
            stale_until: stored.stale_until,
            retry_after: 0,
            bytes,
        };
        self.remember(key, entry.clone()).await;
        Some(entry)
    }

    async fn persist(&self, db: &SqlitePool, key: &str, entry: &Entry) -> Result<(), sqlx::Error> {
        let (status, payload) = match &entry.result {
            Ok(value) => (200, Some(value.to_string())),
            Err(error) if error.0 == StatusCode::NOT_FOUND => (404, None),
            // Network failures belong only in the short in-memory retry cooldown.
            Err(_) => return Ok(()),
        };
        if entry.bytes > MAX_PAYLOAD_BYTES {
            // A newer oversized response must not leave an obsolete disk entry behind.
            sqlx::query("DELETE FROM catalog_cache WHERE cache_key = ?")
                .bind(key)
                .execute(db)
                .await?;
            return Ok(());
        }
        let mut tx = db.begin().await?;
        sqlx::query("DELETE FROM catalog_cache WHERE stale_until <= ?")
            .bind(now())
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO catalog_cache (cache_key,status,payload,fetched_at,fresh_until,stale_until)
             VALUES (?,?,?,?,?,?) ON CONFLICT(cache_key) DO UPDATE SET
             status=excluded.status, payload=excluded.payload, fetched_at=excluded.fetched_at,
             fresh_until=excluded.fresh_until, stale_until=excluded.stale_until",
        ).bind(key).bind(status).bind(payload).bind(entry.fetched_at)
            .bind(entry.fresh_until).bind(entry.stale_until).execute(&mut *tx).await?;
        sqlx::query(
            "DELETE FROM catalog_cache WHERE cache_key IN (
                SELECT cache_key FROM (
                    SELECT cache_key,
                        ROW_NUMBER() OVER (ORDER BY fetched_at DESC, cache_key DESC) AS position,
                        SUM(COALESCE(length(CAST(payload AS BLOB)),0)) OVER
                            (ORDER BY fetched_at DESC, cache_key DESC) AS total_bytes
                    FROM catalog_cache
                ) WHERE position > ? OR total_bytes > ?
             )",
        )
        .bind(DISK_LIMIT)
        .bind(DISK_BYTES)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
}

pub(crate) async fn fetch(
    state: &AppState,
    url: &str,
    params: &[(&str, &str)],
) -> ServiceResult<Value> {
    let key = key(url, params);
    // Every consumer (details, topics, ISBN and recommendations) shares this lock.
    // Recheck after waiting so simultaneous misses cause just one HTTP request.
    let lock = state.catalog_cache.request_lock(&key).await;
    let _guard = lock.lock().await;
    let cached = state.catalog_cache.load(&state.db, &key).await;
    if let Some(entry) = &cached {
        let timestamp = now();
        if entry.fresh_until > timestamp || entry.retry_after > timestamp {
            return entry.result.clone();
        }
    }
    let result = request(state, url, params).await;
    let timestamp = now();
    if result
        .as_ref()
        .is_err_and(|error| error.0 != StatusCode::NOT_FOUND)
    {
        if let Some(mut entry) =
            cached.filter(|entry| entry.result.is_ok() && entry.stale_until > timestamp)
        {
            entry.retry_after = (timestamp + RETRY_DELAY).min(entry.stale_until);
            let result = entry.result.clone();
            state.catalog_cache.remember(&key, entry).await;
            return result;
        }
    }
    let isbn_search = params
        .iter()
        .any(|(name, value)| *name == "q" && value.starts_with("isbn:"));
    let (fresh, stale) = match &result {
        Ok(_) if url.ends_with("/search.json") && !isbn_search => (DAY, 7 * DAY),
        Ok(_) => (30 * DAY, 90 * DAY),
        Err(error) if error.0 == StatusCode::NOT_FOUND => (NOT_FOUND_TTL, NOT_FOUND_TTL),
        Err(_) => (RETRY_DELAY, RETRY_DELAY),
    };
    let entry = Entry {
        bytes: result.as_ref().map_or(0, |value| value.to_string().len()),
        result: result.clone(),
        fetched_at: timestamp,
        fresh_until: timestamp + fresh,
        stale_until: timestamp + stale,
        retry_after: 0,
    };
    if let Err(error) = state.catalog_cache.persist(&state.db, &key, &entry).await {
        // A cache write must never turn a successful catalog response into an error.
        tracing::warn!(%error, "catalog cache write failed");
    }
    state.catalog_cache.remember(&key, entry).await;
    result
}

async fn request(state: &AppState, url: &str, params: &[(&str, &str)]) -> ServiceResult<Value> {
    {
        let mut last = state.catalog_gate.lock().await;
        let wait = Duration::from_secs(1).saturating_sub(last.elapsed());
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        *last = Instant::now();
    }
    let response = state
        .http
        .get(url)
        .query(params)
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "catalog request failed");
            AppError(
                StatusCode::BAD_GATEWAY,
                "Catálogo temporariamente indisponível".into(),
            )
        })?;
    if !response.status().is_success() {
        tracing::warn!(status=%response.status(), "catalog response failed");
        return Err(if response.status() == StatusCode::NOT_FOUND {
            not_found()
        } else {
            AppError(
                StatusCode::BAD_GATEWAY,
                "O catálogo não respondeu à pesquisa".into(),
            )
        });
    }
    response.json::<Value>().await.map_err(|_| {
        AppError(
            StatusCode::BAD_GATEWAY,
            "Resposta inválida do catálogo".into(),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        task::JoinHandle,
    };

    struct Server {
        url: String,
        count: Arc<AtomicUsize>,
        response: Arc<Mutex<(u16, String)>>,
        task: JoinHandle<()>,
    }

    impl Server {
        async fn start() -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let url = format!("http://{}", listener.local_addr().unwrap());
            let count = Arc::new(AtomicUsize::new(0));
            let response = Arc::new(Mutex::new((
                200,
                json!({"title":"Original book"}).to_string(),
            )));
            let request_count = count.clone();
            let current_response = response.clone();
            let task = tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    let mut request = Vec::new();
                    let mut buffer = [0; 1_024];
                    loop {
                        let read = stream.read(&mut buffer).await.unwrap();
                        if read == 0 {
                            break;
                        }
                        request.extend_from_slice(&buffer[..read]);
                        if request.windows(4).any(|end| end == b"\r\n\r\n") {
                            break;
                        }
                    }
                    request_count.fetch_add(1, Ordering::SeqCst);
                    let (status, body) = current_response.lock().await.clone();
                    let response = format!(
                        "HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len(),
                    );
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            });
            Self {
                url,
                count,
                response,
                task,
            }
        }

        async fn respond(&self, status: u16, value: Value) {
            *self.response.lock().await = (status, value.to_string());
        }

        fn requests(&self) -> usize {
            self.count.load(Ordering::SeqCst)
        }
    }

    impl Drop for Server {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    async fn age(state: &AppState, seconds: i64) {
        state.catalog_cache.memory.lock().await.clear();
        sqlx::query("UPDATE catalog_cache SET fetched_at=fetched_at-?, fresh_until=fresh_until-?, stale_until=stale_until-?")
            .bind(seconds).bind(seconds).bind(seconds).execute(&state.db).await.unwrap();
    }

    #[tokio::test]
    async fn simultaneous_requests_share_one_response_and_survive_restart() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("library.sqlite");
        let state = AppState::open(&path).await.unwrap();
        let server = Server::start().await;
        let url = format!("{}/works/OL1W.json", server.url);
        let mut requests = tokio::task::JoinSet::new();
        for _ in 0..12 {
            let state = state.clone();
            let url = url.clone();
            requests.spawn(async move { fetch(&state, &url, &[]).await.unwrap() });
        }
        while let Some(result) = requests.join_next().await {
            assert_eq!(result.unwrap()["title"], "Original book");
        }
        assert_eq!(server.requests(), 1);
        // Metadata remains fresh well beyond the former 30-minute cache.
        age(&state, 29 * DAY).await;
        state.db.close().await;
        drop(state);
        let reopened = AppState::open(&path).await.unwrap();
        assert_eq!(
            fetch(&reopened, &url, &[]).await.unwrap()["title"],
            "Original book"
        );
        assert_eq!(server.requests(), 1);
    }

    #[tokio::test]
    async fn expired_search_refreshes_and_uses_stale_data_only_for_temporary_failures() {
        let folder = tempfile::tempdir().unwrap();
        let state = AppState::open(&folder.path().join("library.sqlite"))
            .await
            .unwrap();
        let server = Server::start().await;
        let url = format!("{}/search.json", server.url);
        let params = [("q", "Dune"), ("limit", "16")];
        fetch(&state, &url, &params).await.unwrap();
        age(&state, 2 * DAY).await;
        server.respond(200, json!({"title":"Updated book"})).await;
        // Parameter order is irrelevant, but pagination and search terms are not.
        let reordered = [("limit", "16"), ("q", "Dune")];
        assert_eq!(
            fetch(&state, &url, &reordered).await.unwrap()["title"],
            "Updated book"
        );
        assert_eq!(server.requests(), 2);
        age(&state, 2 * DAY).await;
        server.respond(503, json!({"error":"Unavailable"})).await;
        for _ in 0..3 {
            assert_eq!(
                fetch(&state, &url, &params).await.unwrap()["title"],
                "Updated book"
            );
        }
        assert_eq!(server.requests(), 3);
        state.catalog_cache.memory.lock().await.clear();
        server.respond(404, json!({"error":"Removed"})).await;
        assert_eq!(
            fetch(&state, &url, &params).await.unwrap_err().0,
            StatusCode::NOT_FOUND
        );
        state.catalog_cache.memory.lock().await.clear();
        assert_eq!(
            fetch(&state, &url, &params).await.unwrap_err().0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(server.requests(), 4);
    }

    #[tokio::test]
    async fn offline_fallback_and_negative_entries_have_expiry_limits() {
        let folder = tempfile::tempdir().unwrap();
        let state = AppState::open(&folder.path().join("library.sqlite"))
            .await
            .unwrap();
        let server = Server::start().await;
        let url = format!("{}/search.json", server.url);
        fetch(&state, &url, &[]).await.unwrap();
        age(&state, 8 * DAY).await;
        server.respond(502, json!({})).await;
        assert!(fetch(&state, &url, &[]).await.is_err());
        assert!(fetch(&state, &url, &[]).await.is_err());
        assert_eq!(server.requests(), 2);
        let disk_status: i64 = sqlx::query_scalar("SELECT status FROM catalog_cache")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(disk_status, 200); // Temporary failures are never persisted as books.
        state.catalog_cache.memory.lock().await.clear();
        server.respond(404, json!({})).await;
        assert!(fetch(&state, &url, &[]).await.is_err());
        age(&state, NOT_FOUND_TTL + 1).await;
        server.respond(200, json!({"title":"Newly added"})).await;
        assert_eq!(
            fetch(&state, &url, &[]).await.unwrap()["title"],
            "Newly added"
        );
        assert_eq!(server.requests(), 4);
    }

    #[tokio::test]
    async fn pagination_is_separate_and_corrupt_cache_does_not_break_catalog() {
        let folder = tempfile::tempdir().unwrap();
        let state = AppState::open(&folder.path().join("library.sqlite"))
            .await
            .unwrap();
        let server = Server::start().await;
        let url = format!("{}/works/OL1W/editions.json", server.url);
        fetch(&state, &url, &[("offset", "0")]).await.unwrap();
        fetch(&state, &url, &[("offset", "50")]).await.unwrap();
        fetch(&state, &url, &[("offset", "0")]).await.unwrap();
        assert_eq!(server.requests(), 2);
        state.catalog_cache.memory.lock().await.clear();
        sqlx::query("UPDATE catalog_cache SET payload='broken json'")
            .execute(&state.db)
            .await
            .unwrap();
        fetch(&state, &url, &[("offset", "0")]).await.unwrap();
        assert_eq!(server.requests(), 3);
        // Even cache I/O failure must leave the network response usable.
        state.catalog_cache.memory.lock().await.clear();
        sqlx::query("DROP TABLE catalog_cache")
            .execute(&state.db)
            .await
            .unwrap();
        fetch(&state, &url, &[("offset", "0")]).await.unwrap();
        assert_eq!(server.requests(), 4);
    }

    #[tokio::test]
    async fn disk_cache_is_bounded_and_does_not_enter_library_backups() {
        let folder = tempfile::tempdir().unwrap();
        let state = AppState::open(&folder.path().join("library.sqlite"))
            .await
            .unwrap();
        let backup = crate::library::export_data(&state).await.unwrap();
        let timestamp = now();
        for index in 0..=DISK_LIMIT {
            sqlx::query("INSERT INTO catalog_cache VALUES (?,200,'{}',?,?,?)")
                .bind(format!("old-{index}"))
                .bind(timestamp - index - 1)
                .bind(timestamp + DAY)
                .bind(timestamp + 7 * DAY)
                .execute(&state.db)
                .await
                .unwrap();
        }
        let server = Server::start().await;
        let url = format!("{}/search.json", server.url);
        fetch(&state, &url, &[]).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM catalog_cache")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(count, DISK_LIMIT);
        // Prune by total byte size too, even when the number of rows is small.
        sqlx::query("DELETE FROM catalog_cache WHERE cache_key != ?")
            .bind(key(&url, &[]))
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO catalog_cache VALUES ('oversized',200,?,0,?,?)")
            .bind("x".repeat(DISK_BYTES as usize))
            .bind(timestamp + DAY)
            .bind(timestamp + 7 * DAY)
            .execute(&state.db)
            .await
            .unwrap();
        fetch(&state, &url, &[("q", "other")]).await.unwrap();
        let size: i64 = sqlx::query_scalar(
            "SELECT SUM(COALESCE(length(CAST(payload AS BLOB)),0)) FROM catalog_cache",
        )
        .fetch_one(&state.db)
        .await
        .unwrap();
        assert!(size <= DISK_BYTES);
        assert_eq!(crate::library::export_data(&state).await.unwrap(), backup);
    }

    #[tokio::test]
    async fn isbn_metadata_is_reused_and_large_responses_remain_bounded() {
        let folder = tempfile::tempdir().unwrap();
        let state = AppState::open(&folder.path().join("library.sqlite"))
            .await
            .unwrap();
        let server = Server::start().await;
        let url = format!("{}/search.json", server.url);
        let params = [("q", "isbn:9780140328721")];
        fetch(&state, &url, &params).await.unwrap();
        age(&state, 29 * DAY).await;
        fetch(&state, &url, &params).await.unwrap();
        assert_eq!(server.requests(), 1);
        age(&state, 2 * DAY).await;
        let oversized = json!({"title":"x".repeat(MAX_PAYLOAD_BYTES + 1)});
        server.respond(200, oversized.clone()).await;
        assert_eq!(fetch(&state, &url, &params).await.unwrap(), oversized);
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM catalog_cache")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(count, 0);
        // The response can still be reused in memory within its separate byte budget.
        assert_eq!(fetch(&state, &url, &params).await.unwrap(), oversized);
        assert_eq!(server.requests(), 2);
        for index in 0..5 {
            let mut entry = state
                .catalog_cache
                .load(&state.db, &key(&url, &params))
                .await
                .unwrap();
            entry.result = Ok(oversized.clone());
            state
                .catalog_cache
                .remember(&format!("large-{index}"), entry)
                .await;
        }
        let memory = state.catalog_cache.memory.lock().await;
        assert!(memory.values().map(|(_, entry)| entry.bytes).sum::<usize>() <= MEMORY_BYTES);
        assert!(memory.len() <= MEMORY_LIMIT);
    }
}
