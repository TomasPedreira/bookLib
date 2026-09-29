use crate::{bad, missing, ApiResult, AppError, AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, SqlitePool};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize, FromRow)]
pub struct Book {
    id: i64,
    title: String,
    authors: String,
    isbn: Option<String>,
    work_id: Option<String>,
    edition_id: Option<String>,
    cover_url: Option<String>,
    page_count: Option<i64>,
    language: Option<String>,
    published: Option<String>,
    description: String,
    source: String,
    status: String,
    #[sqlx(rename = "user_rating")]
    rating: Option<i64>,
    review: String,
    notes: String,
    tags: String,
    #[serde(default)]
    topics: String,
    created_at: String,
    updated_at: String,
}
#[derive(Clone, Serialize, Deserialize, FromRow)]
pub struct Reading {
    id: i64,
    book_id: i64,
    status: String,
    unit: String,
    current_value: i64,
    started_at: String,
    finished_at: Option<String>,
    created_at: String,
}
#[derive(Clone, Serialize, Deserialize, FromRow)]
pub struct Progress {
    id: i64,
    reading_id: i64,
    value: i64,
    note: String,
    recorded_at: String,
}
#[derive(Clone, Deserialize)]
pub struct BookInput {
    title: String,
    authors: String,
    isbn: Option<String>,
    work_id: Option<String>,
    edition_id: Option<String>,
    cover_url: Option<String>,
    page_count: Option<i64>,
    language: Option<String>,
    published: Option<String>,
    description: Option<String>,
    source: Option<String>,
    rating: Option<i64>,
    review: Option<String>,
    notes: Option<String>,
    tags: Option<String>,
    topics: Option<String>,
}
#[derive(Deserialize)]
pub struct RatingInput {
    rating: Option<i64>,
}
#[derive(Deserialize)]
pub struct StartReading {
    unit: String,
}
#[derive(Deserialize)]
pub struct ReadingStatus {
    status: String,
}
#[derive(Deserialize)]
pub struct ProgressInput {
    value: i64,
    note: Option<String>,
    recorded_at: Option<String>,
}
#[derive(Deserialize)]
pub struct Backup {
    version: i64,
    books: Vec<Book>,
    readings: Vec<Reading>,
    progress_entries: Vec<Progress>,
}

async fn book(db: &SqlitePool, id: i64) -> Result<Book, AppError> {
    sqlx::query_as::<_, Book>("SELECT * FROM books WHERE id=?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(missing)
}
async fn reading(db: &SqlitePool, id: i64) -> Result<Reading, AppError> {
    sqlx::query_as::<_, Reading>("SELECT * FROM readings WHERE id=?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(missing)
}
async fn progress(db: &SqlitePool, id: i64) -> Result<Progress, AppError> {
    sqlx::query_as::<_, Progress>("SELECT * FROM progress_entries WHERE id=?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or_else(missing)
}
fn valid_olid(s: &str, suffix: char) -> bool {
    let Some(mid) = s.strip_prefix("OL").and_then(|x| x.strip_suffix(suffix)) else {
        return false;
    };
    !mid.is_empty() && mid.len() <= 12 && mid.bytes().all(|b| b.is_ascii_digit())
}
fn validate_book(b: &BookInput) -> Result<(), AppError> {
    if b.title.trim().is_empty() {
        return Err(bad("Indica o título"));
    }
    if b.title.len() > 300 || b.authors.len() > 300 {
        return Err(bad("Título ou autor demasiado longo"));
    }
    if b.page_count.is_some_and(|n| !(1..=100_000).contains(&n)) {
        return Err(bad("Número de páginas inválido"));
    }
    if b.rating.is_some_and(|n| !(1..=10).contains(&n)) {
        return Err(bad("Classificação inválida"));
    }
    if let Some(topics) = b.topics.as_deref() {
        let selected: Vec<&str> = topics.split(',').filter(|topic| !topic.is_empty()).collect();
        if selected.len() > 4
            || selected.iter().any(|topic| !crate::catalog::TOPICS.contains(topic))
            || selected.iter().enumerate().any(|(index, topic)| selected[..index].contains(topic))
        {
            return Err(bad("Temas inválidos"));
        }
    }
    if b.edition_id.as_deref().is_some_and(|s| !valid_olid(s, 'M')) {
        return Err(bad("Identificador de edição inválido"));
    }
    if b.work_id.as_deref().is_some_and(|s| !valid_olid(s, 'W')) {
        return Err(bad("Identificador de obra inválido"));
    }
    if b.isbn.as_deref().is_some_and(|s| s.len() > 32)
        || b.language.as_deref().is_some_and(|s| s.len() > 80)
        || b.published.as_deref().is_some_and(|s| s.len() > 120)
        || b.source.as_deref().is_some_and(|s| s.len() > 40)
        || b.tags.as_deref().is_some_and(|s| s.len() > 2_000)
        || b.topics.as_deref().is_some_and(|s| s.len() > 400)
    {
        return Err(bad("Campo do livro demasiado longo"));
    }
    if b.cover_url.as_deref().is_some_and(|s| {
        s.len() > 2_000
            || !reqwest::Url::parse(s).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
    }) {
        return Err(bad("URL da capa inválido"));
    }
    if b.description.as_deref().unwrap_or("").len() > 10_000
        || b.review.as_deref().unwrap_or("").len() > 10_000
        || b.notes.as_deref().unwrap_or("").len() > 10_000
    {
        return Err(bad("Texto demasiado longo"));
    }
    Ok(())
}

pub async fn list_books(State(s): State<AppState>) -> ApiResult<Vec<Value>> {
    let books = sqlx::query_as::<_, Book>("SELECT * FROM books ORDER BY updated_at DESC, id DESC")
        .fetch_all(&s.db)
        .await?;
    let readings = sqlx::query_as::<_, Reading>("SELECT r.* FROM readings r WHERE r.id = (SELECT id FROM readings WHERE book_id = r.book_id ORDER BY id DESC LIMIT 1)")
        .fetch_all(&s.db)
        .await?;
    let mut latest = HashMap::new();
    for r in readings {
        latest.entry(r.book_id).or_insert(r);
    }
    Ok(Json(
        books
            .into_iter()
            .map(|b| {
                let r = latest.remove(&b.id);
                json!({"book": b, "reading": r})
            })
            .collect(),
    ))
}
pub async fn get_book(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Value> {
    let b = book(&s.db, id).await?;
    let readings =
        sqlx::query_as::<_, Reading>("SELECT * FROM readings WHERE book_id=? ORDER BY id DESC")
            .bind(id)
            .fetch_all(&s.db)
            .await?;
    let mut history = Vec::new();
    for r in readings {
        let entries = sqlx::query_as::<_, Progress>(
            "SELECT * FROM progress_entries WHERE reading_id=? ORDER BY recorded_at DESC, id DESC",
        )
        .bind(r.id)
        .fetch_all(&s.db)
        .await?;
        history.push(json!({"reading": r, "progress": entries}));
    }
    Ok(Json(json!({"book": b, "readings": history})))
}
pub async fn create_book(State(s): State<AppState>, Json(b): Json<BookInput>) -> ApiResult<Book> {
    validate_book(&b)?;
    if let Some(ref id) = b.edition_id {
        if sqlx::query_scalar::<_, i64>("SELECT id FROM books WHERE edition_id=?")
            .bind(id)
            .fetch_optional(&s.db)
            .await?
            .is_some()
        {
            return Err(bad("Esta edição já está na biblioteca"));
        }
    }
    let topics = match (&b.topics, &b.work_id) {
        (Some(topics), _) => topics.clone(),
        (None, Some(work_id)) => match crate::catalog::default_book_topics(&s, work_id).await {
            Ok(topics) => topics,
            Err(error) => {
                tracing::warn!(?error, %work_id, "could not load book topics");
                String::new()
            }
        },
        _ => String::new(),
    };
    let id = sqlx::query("INSERT INTO books (title,authors,isbn,work_id,edition_id,cover_url,page_count,language,published,description,source,user_rating,review,notes,tags,topics) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(b.title.trim()).bind(b.authors.trim()).bind(b.isbn).bind(b.work_id).bind(b.edition_id)
        .bind(b.cover_url).bind(b.page_count).bind(b.language).bind(b.published)
        .bind(b.description.unwrap_or_default()).bind(b.source.unwrap_or_else(|| "manual".into()))
        .bind(b.rating).bind(b.review.unwrap_or_default()).bind(b.notes.unwrap_or_default())
        .bind(b.tags.unwrap_or_default()).bind(topics).execute(&s.db).await?.last_insert_rowid();
    Ok(Json(book(&s.db, id).await?))
}
pub async fn update_book(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(b): Json<BookInput>,
) -> ApiResult<Book> {
    book(&s.db, id).await?;
    validate_book(&b)?;
    sqlx::query("UPDATE books SET title=?,authors=?,isbn=?,work_id=?,edition_id=?,cover_url=?,page_count=?,language=?,published=?,description=?,user_rating=?,review=?,notes=?,tags=?,topics=?,updated_at=datetime('now') WHERE id=?")
        .bind(b.title.trim()).bind(b.authors.trim()).bind(b.isbn).bind(b.work_id).bind(b.edition_id)
        .bind(b.cover_url).bind(b.page_count).bind(b.language).bind(b.published)
        .bind(b.description.unwrap_or_default()).bind(b.rating).bind(b.review.unwrap_or_default())
        .bind(b.notes.unwrap_or_default()).bind(b.tags.unwrap_or_default()).bind(b.topics.unwrap_or_default()).bind(id).execute(&s.db).await?;
    Ok(Json(book(&s.db, id).await?))
}
pub async fn update_rating(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<RatingInput>,
) -> ApiResult<Book> {
    if input.rating.is_some_and(|rating| !(1..=10).contains(&rating)) {
        return Err(bad("Classificação inválida"));
    }
    book(&s.db, id).await?;
    sqlx::query("UPDATE books SET user_rating=?,updated_at=datetime('now') WHERE id=?")
        .bind(input.rating)
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(Json(book(&s.db, id).await?))
}
pub async fn delete_book(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Value> {
    book(&s.db, id).await?;
    sqlx::query("DELETE FROM books WHERE id=?")
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}
pub async fn start_reading(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<StartReading>,
) -> ApiResult<Reading> {
    let b = book(&s.db, id).await?;
    if !["pages", "percent"].contains(&input.unit.as_str()) {
        return Err(bad("Unidade inválida"));
    }
    if input.unit == "pages" && b.page_count.is_none() {
        return Err(bad("Indica o número de páginas ou usa percentagem"));
    }
    let latest = sqlx::query_as::<_, Reading>(
        "SELECT * FROM readings WHERE book_id=? ORDER BY id DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?;
    if latest.is_some_and(|r| r.status == "reading" || r.status == "paused") {
        return Err(bad("Já existe uma leitura em curso"));
    }
    let mut tx = s.db.begin().await?;
    let rid = sqlx::query("INSERT INTO readings (book_id,status,unit) VALUES (?,'reading',?)")
        .bind(id)
        .bind(input.unit)
        .execute(&mut *tx)
        .await?
        .last_insert_rowid();
    sqlx::query("UPDATE books SET status='reading',updated_at=datetime('now') WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(reading(&s.db, rid).await?))
}
pub async fn change_reading(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<ReadingStatus>,
) -> ApiResult<Reading> {
    let r = reading(&s.db, id).await?;
    if !["reading", "paused", "completed", "abandoned"].contains(&input.status.as_str()) {
        return Err(bad("Estado inválido"));
    }
    if r.status == "completed" || r.status == "abandoned" {
        return Err(bad("Esta leitura já terminou; inicia uma releitura"));
    }
    let latest: i64 =
        sqlx::query_scalar("SELECT id FROM readings WHERE book_id=? ORDER BY id DESC LIMIT 1")
            .bind(r.book_id)
            .fetch_one(&s.db)
            .await?;
    if latest != id {
        return Err(bad("Só podes alterar a leitura mais recente"));
    }
    let mut tx = s.db.begin().await?;
    sqlx::query("UPDATE readings SET status=?,finished_at=CASE WHEN ? IN ('completed','abandoned') THEN date('now') ELSE NULL END WHERE id=?")
        .bind(&input.status).bind(&input.status).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE books SET status=?,updated_at=datetime('now') WHERE id=?")
        .bind(&input.status)
        .bind(r.book_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(reading(&s.db, id).await?))
}

async fn validate_progress(db: &SqlitePool, id: i64, value: i64) -> Result<Reading, AppError> {
    let r = reading(db, id).await?;
    let max = if r.unit == "percent" {
        100
    } else {
        book(db, r.book_id).await?.page_count.unwrap_or(0)
    };
    if value < 0 || value > max {
        return Err(bad(format!("Indica um valor entre 0 e {max}")));
    }
    Ok(r)
}
fn is_date(d: &str) -> bool {
    let bytes = d.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let year: u32 = d[..4].parse().unwrap_or(0);
    let month: u32 = d[5..7].parse().unwrap_or(0);
    let day: u32 = d[8..10].parse().unwrap_or(0);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    year > 0 && day > 0 && day <= days
}
fn is_timestamp(value: &str) -> bool {
    if is_date(value) {
        return true;
    }
    let bytes = value.as_bytes();
    if bytes.len() != 19
        || bytes[10] != b' '
        || bytes[13] != b':'
        || bytes[16] != b':'
        || !is_date(&value[..10])
        || !bytes[11..]
            .iter()
            .enumerate()
            .all(|(i, c)| i == 2 || i == 5 || c.is_ascii_digit())
    {
        return false;
    }
    let hour: u32 = value[11..13].parse().unwrap_or(24);
    let minute: u32 = value[14..16].parse().unwrap_or(60);
    let second: u32 = value[17..19].parse().unwrap_or(60);
    hour < 24 && minute < 60 && second < 60
}
fn valid_date(date: Option<String>) -> Result<Option<String>, AppError> {
    if date.as_deref().is_some_and(|d| !is_date(d)) {
        return Err(bad("Data inválida"));
    }
    Ok(date)
}
async fn recalc(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, id: i64) -> Result<(), sqlx::Error> {
    let value = sqlx::query_scalar::<_, i64>("SELECT value FROM progress_entries WHERE reading_id=? ORDER BY recorded_at DESC,id DESC LIMIT 1")
        .bind(id).fetch_optional(&mut **tx).await?.unwrap_or(0);
    sqlx::query("UPDATE readings SET current_value=? WHERE id=?")
        .bind(value)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
pub async fn add_progress(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<ProgressInput>,
) -> ApiResult<Progress> {
    let r = validate_progress(&s.db, id, input.value).await?;
    if input
        .note
        .as_deref()
        .is_some_and(|note| note.len() > 10_000)
    {
        return Err(bad("Texto demasiado longo"));
    }
    if r.status != "reading" && r.status != "paused" {
        return Err(bad("Esta leitura já terminou"));
    }
    let date = valid_date(input.recorded_at)?;
    let mut tx = s.db.begin().await?;
    let pid = sqlx::query("INSERT INTO progress_entries (reading_id,value,note,recorded_at) VALUES (?,?,?,COALESCE(?,datetime('now')))")
        .bind(id).bind(input.value).bind(input.note.unwrap_or_default()).bind(date)
        .execute(&mut *tx).await?.last_insert_rowid();
    recalc(&mut tx, id).await?;
    sqlx::query("UPDATE books SET updated_at=datetime('now') WHERE id=?")
        .bind(r.book_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(progress(&s.db, pid).await?))
}
pub async fn edit_progress(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(input): Json<ProgressInput>,
) -> ApiResult<Progress> {
    let p = progress(&s.db, id).await?;
    validate_progress(&s.db, p.reading_id, input.value).await?;
    if input
        .note
        .as_deref()
        .is_some_and(|note| note.len() > 10_000)
    {
        return Err(bad("Texto demasiado longo"));
    }
    let date = valid_date(input.recorded_at)?;
    let mut tx = s.db.begin().await?;
    sqlx::query(
        "UPDATE progress_entries SET value=?,note=?,recorded_at=COALESCE(?,recorded_at) WHERE id=?",
    )
    .bind(input.value)
    .bind(input.note.unwrap_or_default())
    .bind(date)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    recalc(&mut tx, p.reading_id).await?;
    tx.commit().await?;
    Ok(Json(progress(&s.db, id).await?))
}
pub async fn delete_progress(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Value> {
    let p = progress(&s.db, id).await?;
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM progress_entries WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    recalc(&mut tx, p.reading_id).await?;
    tx.commit().await?;
    Ok(Json(json!({"ok": true})))
}
pub async fn stats(State(s): State<AppState>) -> ApiResult<Value> {
    let (total, want, reading, paused, completed, abandoned): (i64, i64, i64, i64, i64, i64) =
        sqlx::query_as("SELECT count(*), count(*) FILTER (WHERE status='want'), count(*) FILTER (WHERE status='reading'), count(*) FILTER (WHERE status='paused'), count(*) FILTER (WHERE status='completed'), count(*) FILTER (WHERE status='abandoned') FROM books")
            .fetch_one(&s.db).await?;
    let reading_sessions: i64 = sqlx::query_scalar("SELECT count(*) FROM readings")
        .fetch_one(&s.db)
        .await?;
    let daily_pages: Vec<(String, i64)> = sqlx::query_as(
        "WITH last_per_day AS (
            SELECT p.reading_id, substr(p.recorded_at, 1, 10) AS day, p.value,
                   ROW_NUMBER() OVER (PARTITION BY p.reading_id, substr(p.recorded_at, 1, 10)
                                      ORDER BY p.recorded_at DESC, p.id DESC) AS position
            FROM progress_entries p JOIN readings r ON r.id = p.reading_id
            WHERE r.unit = 'pages'
         ), changes AS (
            SELECT day, value,
                   LAG(value, 1, 0) OVER (PARTITION BY reading_id ORDER BY day) AS previous
            FROM last_per_day WHERE position = 1
         )
         SELECT day, SUM(CASE WHEN value > previous THEN value - previous ELSE 0 END) AS pages
         FROM changes GROUP BY day ORDER BY day"
    ).fetch_all(&s.db).await?;
    let pages: i64 = daily_pages.iter().map(|(_, count)| count).sum();
    let daily_pages: Vec<Value> = daily_pages.into_iter()
        .map(|(date, pages)| json!({"date": date, "pages": pages}))
        .collect();
    Ok(Json(json!({
        "total": total, "want": want, "reading": reading,
        "paused": paused, "completed": completed,
        "abandoned": abandoned,
        "reading_sessions": reading_sessions, "pages_read": pages,
        "daily_pages": daily_pages
    })))
}
pub async fn export_data(State(s): State<AppState>) -> ApiResult<Value> {
    let books = sqlx::query_as::<_, Book>("SELECT * FROM books ORDER BY id")
        .fetch_all(&s.db)
        .await?;
    let readings = sqlx::query_as::<_, Reading>("SELECT * FROM readings ORDER BY id")
        .fetch_all(&s.db)
        .await?;
    let progress_entries =
        sqlx::query_as::<_, Progress>("SELECT * FROM progress_entries ORDER BY id")
            .fetch_all(&s.db)
            .await?;
    Ok(Json(
        json!({"version": 2, "books": books, "readings": readings, "progress_entries": progress_entries}),
    ))
}
pub async fn import_data(State(s): State<AppState>, Json(data): Json<Backup>) -> ApiResult<Value> {
    if data.version != 1 && data.version != 2 {
        return Err(bad("Versão de ficheiro não suportada"));
    }
    if data.books.len() > 100_000
        || data.readings.len() > 500_000
        || data.progress_entries.len() > 2_000_000
    {
        return Err(bad("Ficheiro demasiado grande"));
    }
    for b in &data.books {
        let input = BookInput {
            title: b.title.clone(),
            authors: b.authors.clone(),
            isbn: b.isbn.clone(),
            work_id: b.work_id.clone(),
            edition_id: b.edition_id.clone(),
            cover_url: b.cover_url.clone(),
            page_count: b.page_count,
            language: b.language.clone(),
            published: b.published.clone(),
            description: Some(b.description.clone()),
            source: Some(b.source.clone()),
            rating: b.rating.map(|rating| if data.version == 1 { rating * 2 } else { rating }),
            review: Some(b.review.clone()),
            notes: Some(b.notes.clone()),
            tags: Some(b.tags.clone()),
            topics: Some(b.topics.clone()),
        };
        validate_book(&input)?;
        if b.id <= 0
            || !["want", "reading", "paused", "completed", "abandoned"].contains(&b.status.as_str())
            || !is_timestamp(&b.created_at)
            || !is_timestamp(&b.updated_at)
        {
            return Err(bad("Dados inválidos no ficheiro"));
        }
    }
    for r in &data.readings {
        if r.id <= 0
            || r.book_id <= 0
            || !["reading", "paused", "completed", "abandoned"].contains(&r.status.as_str())
            || !["pages", "percent"].contains(&r.unit.as_str())
            || r.current_value < 0
            || !is_date(&r.started_at)
            || r.finished_at.as_deref().is_some_and(|d| !is_date(d))
            || !is_timestamp(&r.created_at)
        {
            return Err(bad("Dados inválidos no ficheiro"));
        }
    }
    for p in &data.progress_entries {
        if p.id <= 0
            || p.reading_id <= 0
            || p.value < 0
            || p.note.len() > 10_000
            || !is_timestamp(&p.recorded_at)
        {
            return Err(bad("Dados inválidos no ficheiro"));
        }
    }
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM progress_entries")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM readings")
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM books").execute(&mut *tx).await?;
    for b in &data.books {
        sqlx::query("INSERT INTO books (id,title,authors,isbn,work_id,edition_id,cover_url,page_count,language,published,description,source,status,user_rating,review,notes,tags,topics,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
            .bind(b.id).bind(&b.title).bind(&b.authors).bind(&b.isbn).bind(&b.work_id).bind(&b.edition_id)
            .bind(&b.cover_url).bind(b.page_count).bind(&b.language).bind(&b.published).bind(&b.description)
            .bind(&b.source).bind(&b.status).bind(b.rating.map(|rating| if data.version == 1 { rating * 2 } else { rating })).bind(&b.review).bind(&b.notes).bind(&b.tags).bind(&b.topics)
            .bind(&b.created_at).bind(&b.updated_at).execute(&mut *tx).await?;
    }
    for r in &data.readings {
        sqlx::query("INSERT INTO readings (id,book_id,status,unit,current_value,started_at,finished_at,created_at) VALUES (?,?,?,?,?,?,?,?)")
            .bind(r.id).bind(r.book_id).bind(&r.status).bind(&r.unit).bind(r.current_value)
            .bind(&r.started_at).bind(&r.finished_at).bind(&r.created_at).execute(&mut *tx).await?;
    }
    for p in &data.progress_entries {
        sqlx::query("INSERT INTO progress_entries (id,reading_id,value,note,recorded_at) VALUES (?,?,?,?,?)")
            .bind(p.id).bind(p.reading_id).bind(p.value).bind(&p.note).bind(&p.recorded_at).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(Json(json!({"ok": true, "books": data.books.len()})))
}

#[cfg(test)]
mod tests {
    use super::{is_date, is_timestamp};

    #[test]
    fn validates_calendar_dates_and_backup_timestamps() {
        assert!(is_date("2024-02-29"));
        assert!(!is_date("2025-02-29"));
        assert!(!is_date("2026-13-01"));
        assert!(!is_date("2026-09-31"));
        assert!(is_timestamp("2026-09-29 23:59:59"));
        assert!(!is_timestamp("2026-09-29 24:00:00"));
        assert!(!is_timestamp("<script>bad</script>"));
    }
}
