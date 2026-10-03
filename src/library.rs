use crate::{bad, missing, AppError, AppState, ServiceResult};
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
        let selected: Vec<&str> = topics
            .split(',')
            .filter(|topic| !topic.is_empty())
            .collect();
        if selected.len() > 4
            || selected
                .iter()
                .any(|topic| !crate::catalog::TOPICS.contains(topic))
            || selected
                .iter()
                .enumerate()
                .any(|(index, topic)| selected[..index].contains(topic))
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

pub async fn list_books(s: &AppState) -> ServiceResult<Vec<Value>> {
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
    Ok(books
        .into_iter()
        .map(|b| {
            let r = latest.remove(&b.id);
            json!({"book": b, "reading": r})
        })
        .collect())
}
pub async fn get_book(s: &AppState, id: i64) -> ServiceResult<Value> {
    let b = book(&s.db, id).await?;
    let readings =
        sqlx::query_as::<_, Reading>("SELECT * FROM readings WHERE book_id=? ORDER BY id DESC")
            .bind(id)
            .fetch_all(&s.db)
            .await?;
    let mut history = Vec::new();
    for r in readings {
        let entries = sqlx::query_as::<_, Progress>(
            "SELECT * FROM progress_entries WHERE reading_id=? ORDER BY substr(recorded_at,1,10) DESC, id DESC",
        )
        .bind(r.id)
        .fetch_all(&s.db)
        .await?;
        history.push(json!({"reading": r, "progress": entries}));
    }
    Ok(json!({"book": b, "readings": history}))
}
pub async fn create_book(s: &AppState, b: BookInput) -> ServiceResult<Book> {
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
        (None, Some(work_id)) => match crate::catalog::default_book_topics(s, work_id).await {
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
    Ok(book(&s.db, id).await?)
}
pub async fn update_book(s: &AppState, id: i64, b: BookInput) -> ServiceResult<Book> {
    book(&s.db, id).await?;
    validate_book(&b)?;
    let mut tx = s.db.begin().await?;
    let (page_readings, largest_value): (i64, i64) = sqlx::query_as(
        "SELECT count(*), COALESCE(MAX(MAX(r.current_value, COALESCE((SELECT MAX(p.value) FROM progress_entries p WHERE p.reading_id=r.id),0))),0) FROM readings r WHERE r.book_id=? AND r.unit='pages'",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if page_readings > 0 {
        match b.page_count {
            None => {
                return Err(bad(
                    "Não podes remover o número de páginas de um livro com leituras em páginas",
                ))
            }
            Some(count) if count < largest_value => {
                return Err(bad(
                    "O número de páginas não pode ser inferior ao progresso registado",
                ))
            }
            _ => {}
        }
    }
    sqlx::query("UPDATE books SET title=?,authors=?,isbn=?,work_id=?,edition_id=?,cover_url=?,page_count=?,language=?,published=?,description=?,user_rating=?,review=?,notes=?,tags=?,topics=?,updated_at=datetime('now') WHERE id=?")
        .bind(b.title.trim()).bind(b.authors.trim()).bind(b.isbn).bind(b.work_id).bind(b.edition_id)
        .bind(b.cover_url).bind(b.page_count).bind(b.language).bind(b.published)
        .bind(b.description.unwrap_or_default()).bind(b.rating).bind(b.review.unwrap_or_default())
        .bind(b.notes.unwrap_or_default()).bind(b.tags.unwrap_or_default()).bind(b.topics.unwrap_or_default()).bind(id).execute(&mut *tx).await?;
    complete_finished_progress(&mut tx).await?;
    tx.commit().await?;
    Ok(book(&s.db, id).await?)
}
pub async fn update_rating(s: &AppState, id: i64, input: RatingInput) -> ServiceResult<Book> {
    if input
        .rating
        .is_some_and(|rating| !(1..=10).contains(&rating))
    {
        return Err(bad("Classificação inválida"));
    }
    book(&s.db, id).await?;
    sqlx::query("UPDATE books SET user_rating=?,updated_at=datetime('now') WHERE id=?")
        .bind(input.rating)
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(book(&s.db, id).await?)
}
pub async fn delete_book(s: &AppState, id: i64) -> ServiceResult<Value> {
    book(&s.db, id).await?;
    sqlx::query("DELETE FROM books WHERE id=?")
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(json!({"ok": true}))
}
pub async fn start_reading(s: &AppState, id: i64, input: StartReading) -> ServiceResult<Reading> {
    let b = book(&s.db, id).await?;
    if input.unit != "pages" {
        return Err(bad("Unidade inválida"));
    }
    if b.page_count.is_none() {
        return Err(bad("Indica o número de páginas"));
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
    Ok(reading(&s.db, rid).await?)
}
async fn complete_finished_progress(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> Result<(), sqlx::Error> {
    for statement in include_str!("../migrations/20261003010000_pages_only.sql").split(';') {
        if !statement.trim().is_empty() {
            sqlx::query(statement).execute(&mut **tx).await?;
        }
    }
    sqlx::query("INSERT INTO progress_entries (reading_id,value,recorded_at)
        SELECT r.id, CASE WHEN r.unit='percent' THEN 100 ELSE b.page_count END,
            max(COALESCE(r.finished_at,date('now')), COALESCE((SELECT max(substr(recorded_at,1,10)) FROM progress_entries WHERE reading_id=r.id),'0001-01-01'))
        FROM readings r JOIN books b ON b.id=r.book_id
        WHERE r.status='completed' AND (r.unit='percent' OR b.page_count IS NOT NULL)
            AND NOT EXISTS (SELECT 1 FROM progress_entries p WHERE p.reading_id=r.id
                AND p.value=CASE WHEN r.unit='percent' THEN 100 ELSE b.page_count END)")
        .execute(&mut **tx).await?;
    sqlx::query("UPDATE readings SET current_value=CASE WHEN unit='percent' THEN 100
        ELSE (SELECT page_count FROM books WHERE id=readings.book_id) END
        WHERE status='completed' AND (unit='percent' OR (SELECT page_count FROM books WHERE id=readings.book_id) IS NOT NULL)")
        .execute(&mut **tx).await?;
    Ok(())
}

pub async fn delete_reading(s: &AppState, id: i64) -> ServiceResult<Value> {
    let mut tx = s.db.begin().await?;
    let r = sqlx::query_as::<_, Reading>("SELECT * FROM readings WHERE id=?")
        .bind(id).fetch_optional(&mut *tx).await?.ok_or_else(missing)?;
    let latest: i64 = sqlx::query_scalar("SELECT id FROM readings WHERE book_id=? ORDER BY id DESC LIMIT 1")
        .bind(r.book_id).fetch_one(&mut *tx).await?;
    if latest != id {
        return Err(bad("Só podes remover a leitura mais recente"));
    }
    sqlx::query("DELETE FROM readings WHERE id=?").bind(id).execute(&mut *tx).await?;
    let previous: Option<String> = sqlx::query_scalar("SELECT status FROM readings WHERE book_id=? ORDER BY id DESC LIMIT 1")
        .bind(r.book_id).fetch_optional(&mut *tx).await?;
    sqlx::query("UPDATE books SET status=?,updated_at=datetime('now') WHERE id=?")
        .bind(previous.unwrap_or_else(|| "want".into())).bind(r.book_id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(json!({"ok": true}))
}

pub async fn change_reading(s: &AppState, id: i64, input: ReadingStatus) -> ServiceResult<Reading> {
    let r = reading(&s.db, id).await?;
    if !["reading", "paused", "completed", "abandoned"].contains(&input.status.as_str()) {
        return Err(bad("Estado inválido"));
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
    if input.status == "completed" {
        complete_finished_progress(&mut tx).await?;
    }
    sqlx::query("UPDATE books SET status=?,updated_at=datetime('now') WHERE id=?")
        .bind(&input.status)
        .bind(r.book_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(reading(&s.db, id).await?)
}

async fn validate_progress(db: &SqlitePool, id: i64, value: i64) -> Result<Reading, AppError> {
    let r = reading(db, id).await?;
    if r.unit != "pages" { return Err(bad("Indica o número de páginas")); }
    let max = book(db, r.book_id).await?.page_count.unwrap_or(0);
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
    let value = sqlx::query_scalar::<_, i64>("SELECT value FROM progress_entries WHERE reading_id=? ORDER BY substr(recorded_at,1,10) DESC,id DESC LIMIT 1")
        .bind(id).fetch_optional(&mut **tx).await?.unwrap_or(0);
    sqlx::query("UPDATE readings SET current_value=? WHERE id=?")
        .bind(value)
        .bind(id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("UPDATE readings SET current_value=CASE WHEN unit='percent' THEN 100
        ELSE (SELECT page_count FROM books WHERE id=readings.book_id) END
        WHERE id=? AND status='completed' AND (unit='percent' OR (SELECT page_count FROM books WHERE id=readings.book_id) IS NOT NULL)")
        .bind(id).execute(&mut **tx).await?;
    Ok(())
}
pub async fn add_progress(s: &AppState, id: i64, input: ProgressInput) -> ServiceResult<Progress> {
    let r = validate_progress(&s.db, id, input.value).await?;
    if input
        .note
        .as_deref()
        .is_some_and(|note| note.len() > 10_000)
    {
        return Err(bad("Texto demasiado longo"));
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
    Ok(progress(&s.db, pid).await?)
}
pub async fn edit_progress(s: &AppState, id: i64, input: ProgressInput) -> ServiceResult<Progress> {
    let p = progress(&s.db, id).await?;
    let r = validate_progress(&s.db, p.reading_id, input.value).await?;
    if input
        .note
        .as_deref()
        .is_some_and(|note| note.len() > 10_000)
    {
        return Err(bad("Texto demasiado longo"));
    }
    let date = valid_date(input.recorded_at)?;
    let total = if r.unit == "percent" { 100 } else { book(&s.db, r.book_id).await?.page_count.unwrap_or(0) };
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
    if r.status == "completed" && input.value == total {
        sqlx::query("UPDATE readings SET finished_at=(SELECT substr(recorded_at,1,10) FROM progress_entries WHERE id=?) WHERE id=?")
            .bind(id).bind(r.id).execute(&mut *tx).await?;
    }
    recalc(&mut tx, p.reading_id).await?;
    tx.commit().await?;
    Ok(progress(&s.db, id).await?)
}
pub async fn delete_progress(s: &AppState, id: i64) -> ServiceResult<Value> {
    let p = progress(&s.db, id).await?;
    let mut tx = s.db.begin().await?;
    sqlx::query("DELETE FROM progress_entries WHERE id=?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    recalc(&mut tx, p.reading_id).await?;
    tx.commit().await?;
    Ok(json!({"ok": true}))
}
pub async fn stats(s: &AppState) -> ServiceResult<Value> {
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
                                      ORDER BY p.id DESC) AS position
            FROM progress_entries p JOIN readings r ON r.id = p.reading_id
            WHERE r.unit = 'pages'
         ), changes AS (
            SELECT day, value,
                   LAG(value, 1, 0) OVER (PARTITION BY reading_id ORDER BY day) AS previous
            FROM last_per_day WHERE position = 1
         )
         SELECT day, SUM(CASE WHEN value > previous THEN value - previous ELSE 0 END) AS pages
         FROM changes GROUP BY day ORDER BY day",
    )
    .fetch_all(&s.db)
    .await?;
    let pages: i64 = daily_pages.iter().map(|(_, count)| count).sum();
    let daily_pages: Vec<Value> = daily_pages
        .into_iter()
        .map(|(date, pages)| json!({"date": date, "pages": pages}))
        .collect();
    Ok(json!({
        "total": total, "want": want, "reading": reading,
        "paused": paused, "completed": completed,
        "abandoned": abandoned,
        "reading_sessions": reading_sessions, "pages_read": pages,
        "daily_pages": daily_pages
    }))
}
pub async fn export_data(s: &AppState) -> ServiceResult<Value> {
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
    Ok(
        json!({"version": 2, "books": books, "readings": readings, "progress_entries": progress_entries}),
    )
}
pub async fn import_data(s: &AppState, data: Backup) -> ServiceResult<Value> {
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
            rating: b.rating.map(|rating| {
                if data.version == 1 {
                    rating * 2
                } else {
                    rating
                }
            }),
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
    complete_finished_progress(&mut tx).await?;
    tx.commit().await?;
    Ok(json!({"ok": true, "books": data.books.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::Arc, time::Instant};

    async fn test_state() -> AppState {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&db).await.unwrap();
        AppState {
            db,
            http: reqwest::Client::new(),
            catalog_gate: Arc::new(tokio::sync::Mutex::new(Instant::now())),
            catalog_cache: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        }
    }

    fn book_input(page_count: Option<i64>) -> BookInput {
        serde_json::from_value(json!({
            "title": "Test book", "authors": "", "page_count": page_count
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn legacy_units_convert_to_pages_only_with_a_known_total() {
        let s = test_state().await;
        let b = create_book(&s, book_input(None)).await.unwrap();
        assert!(start_reading(&s, b.id, StartReading { unit: "pages".into() }).await.is_err());
        assert!(start_reading(&s, b.id, StartReading { unit: "percent".into() }).await.is_err());
        let id = sqlx::query("INSERT INTO readings (book_id,status,unit,current_value) VALUES (?,'paused','percent',50)")
            .bind(b.id).execute(&s.db).await.unwrap().last_insert_rowid();
        sqlx::query("INSERT INTO progress_entries (reading_id,value,note,recorded_at) VALUES (?,50,'Preserve me','2026-09-15')")
            .bind(id).execute(&s.db).await.unwrap();
        let data = export_data(&s).await.unwrap();
        import_data(&s, serde_json::from_value(data).unwrap()).await.unwrap();
        assert_eq!(reading(&s.db, id).await.unwrap().unit, "percent");
        update_book(&s, b.id, book_input(Some(529))).await.unwrap();
        let r = reading(&s.db, id).await.unwrap();
        assert_eq!(r.unit, "pages");
        assert_eq!(r.current_value, 265);
        assert_eq!(r.status, "paused");
        let history = get_book(&s, b.id).await.unwrap();
        assert_eq!(history["readings"][0]["progress"][0]["note"], "Preserve me");
        assert_eq!(history["readings"][0]["progress"][0]["recorded_at"], "2026-09-15");
        assert_eq!(stats(&s).await.unwrap()["pages_read"], 265);
        let before = export_data(&s).await.unwrap();
        import_data(&s, serde_json::from_value(before.clone()).unwrap()).await.unwrap();
        assert_eq!(export_data(&s).await.unwrap(), before);
    }

    #[tokio::test]
    async fn closed_states_and_progress_can_be_corrected_in_place() {
        let s = test_state().await;
        let b = create_book(&s, book_input(Some(529))).await.unwrap();
        let r = start_reading(&s, b.id, StartReading { unit: "pages".into() }).await.unwrap();
        let p = add_progress(&s, r.id, ProgressInput { value: 512, note: None, recorded_at: Some("2026-10-01".into()) }).await.unwrap();
        change_reading(&s, r.id, ReadingStatus { status: "abandoned".into() }).await.unwrap();
        edit_progress(&s, p.id, ProgressInput { value: 500, note: None, recorded_at: Some("2026-10-01".into()) }).await.unwrap();
        assert_eq!(reading(&s.db, r.id).await.unwrap().current_value, 500);
        change_reading(&s, r.id, ReadingStatus { status: "paused".into() }).await.unwrap();
        assert_eq!(reading(&s.db, r.id).await.unwrap().current_value, 500);
        change_reading(&s, r.id, ReadingStatus { status: "completed".into() }).await.unwrap();
        assert_eq!(reading(&s.db, r.id).await.unwrap().current_value, 529);
        change_reading(&s, r.id, ReadingStatus { status: "reading".into() }).await.unwrap();
        assert_eq!(book(&s.db, b.id).await.unwrap().status, "reading");
        assert_eq!(get_book(&s, b.id).await.unwrap()["readings"].as_array().unwrap().len(), 1);
        assert_eq!(stats(&s).await.unwrap()["pages_read"], 529);
    }

    #[tokio::test]
    async fn accidental_reread_can_be_removed_without_losing_the_original() {
        let s = test_state().await;
        let mut input = book_input(Some(500));
        input.rating = Some(8);
        let b = create_book(&s, input).await.unwrap();
        let original = start_reading(&s, b.id, StartReading { unit: "pages".into() }).await.unwrap();
        change_reading(&s, original.id, ReadingStatus { status: "completed".into() }).await.unwrap();
        let reread = start_reading(&s, b.id, StartReading { unit: "pages".into() }).await.unwrap();
        change_reading(&s, reread.id, ReadingStatus { status: "completed".into() }).await.unwrap();
        assert_eq!(stats(&s).await.unwrap()["pages_read"], 1000);
        assert!(delete_reading(&s, original.id).await.is_err());
        delete_reading(&s, reread.id).await.unwrap();
        assert_eq!(stats(&s).await.unwrap()["pages_read"], 500);
        assert_eq!(book(&s.db, b.id).await.unwrap().status, "completed");
        assert_eq!(book(&s.db, b.id).await.unwrap().rating, Some(8));
        assert_eq!(get_book(&s, b.id).await.unwrap()["readings"].as_array().unwrap().len(), 1);
        delete_reading(&s, original.id).await.unwrap();
        assert_eq!(book(&s.db, b.id).await.unwrap().status, "want");
        assert_eq!(stats(&s).await.unwrap()["pages_read"], 0);
    }

    #[tokio::test]
    async fn completed_progress_date_can_be_corrected_without_another_reading() {
        let s = test_state().await;
        let b = create_book(&s, book_input(Some(500))).await.unwrap();
        let r = start_reading(&s, b.id, StartReading { unit: "pages".into() }).await.unwrap();
        add_progress(&s, r.id, ProgressInput { value: 100, note: None, recorded_at: Some("2026-10-01".into()) }).await.unwrap();
        change_reading(&s, r.id, ReadingStatus { status: "completed".into() }).await.unwrap();
        let final_id: i64 = sqlx::query_scalar("SELECT id FROM progress_entries WHERE reading_id=? AND value=500").bind(r.id).fetch_one(&s.db).await.unwrap();
        edit_progress(&s, final_id, ProgressInput { value: 500, note: None, recorded_at: Some("2026-09-20".into()) }).await.unwrap();
        assert_eq!(reading(&s.db, r.id).await.unwrap().current_value, 500);
        assert_eq!(reading(&s.db, r.id).await.unwrap().finished_at.as_deref(), Some("2026-09-20"));
        let data = export_data(&s).await.unwrap();
        assert_eq!(data["progress_entries"].as_array().unwrap().len(), 2);
        import_data(&s, serde_json::from_value(data.clone()).unwrap()).await.unwrap();
        assert_eq!(export_data(&s).await.unwrap(), data);
        let st = stats(&s).await.unwrap();
        assert_eq!(st["pages_read"], 500);
        assert_eq!(st["daily_pages"][0], json!({"date":"2026-09-20", "pages":500}));
    }

    #[tokio::test]
    async fn finished_reads_reach_the_total_and_other_statuses_keep_progress() {
        let s = test_state().await;
        for (status, unit, pages, expected) in [
            ("completed", "pages", Some(12), 12),
            ("paused", "pages", Some(12), 5),
            ("abandoned", "pages", Some(12), 5),
        ] {
            let b = create_book(&s, book_input(pages)).await.unwrap();
            let r = start_reading(&s, b.id, StartReading { unit: unit.into() })
                .await
                .unwrap();
            add_progress(
                &s,
                r.id,
                ProgressInput {
                    value: 5,
                    note: None,
                    recorded_at: None,
                },
            )
            .await
            .unwrap();
            let finished = change_reading(
                &s,
                r.id,
                ReadingStatus {
                    status: status.into(),
                },
            )
            .await
            .unwrap();
            assert_eq!(finished.current_value, expected);
            assert_eq!(finished.status, status);
            let detail = get_book(&s, b.id).await.unwrap();
            assert_eq!(
                detail["readings"][0]["progress"].as_array().unwrap().len(),
                if status == "completed" { 2 } else { 1 }
            );
        }
        assert_eq!(stats(&s).await.unwrap()["pages_read"], 22);

        // Old finished backups can contain a partial value. Restoring repairs it.
        let mut backup = export_data(&s).await.unwrap();
        backup["readings"][0]["current_value"] = json!(5);
        backup["progress_entries"]
            .as_array_mut()
            .unwrap()
            .retain(|p| p["value"] != 12);
        import_data(&s, serde_json::from_value(backup).unwrap())
            .await
            .unwrap();
        assert_eq!(reading(&s.db, 1).await.unwrap().current_value, 12);
        let repaired = export_data(&s).await.unwrap();
        sqlx::raw_sql(include_str!(
            "../migrations/20261003000000_finished_progress.sql"
        ))
        .execute(&s.db)
        .await
        .unwrap();
        assert_eq!(export_data(&s).await.unwrap(), repaired);
    }

    #[tokio::test]
    async fn page_count_edits_preserve_reading_history() {
        let s = test_state().await;
        let b = create_book(&s, book_input(Some(300))).await.unwrap();
        let r = start_reading(
            &s,
            b.id,
            StartReading {
                unit: "pages".into(),
            },
        )
        .await
        .unwrap();
        for value in [150, 100] {
            let _ = add_progress(
                &s,
                r.id,
                ProgressInput {
                    value,
                    note: None,
                    recorded_at: Some("2026-10-01".into()),
                },
            )
            .await
            .unwrap();
        }
        let _ = change_reading(
            &s,
            r.id,
            ReadingStatus {
                status: "completed".into(),
            },
        )
        .await
        .unwrap();
        for count in [None, Some(120), Some(150)] {
            let error = update_book(&s, b.id, book_input(count))
                .await
                .err()
                .unwrap();
            assert_eq!(error.0, reqwest::StatusCode::BAD_REQUEST);
            assert_eq!(book(&s.db, b.id).await.unwrap().page_count, Some(300));
        }
        let _ = update_book(&s, b.id, book_input(Some(350))).await.unwrap();
        assert_eq!(book(&s.db, b.id).await.unwrap().page_count, Some(350));
        let other = create_book(&s, book_input(Some(200))).await.unwrap();
        let _ = update_book(&s, other.id, book_input(None)).await.unwrap();
    }

    #[tokio::test]
    async fn mixed_date_formats_use_last_entry_of_each_day() {
        let s = test_state().await;
        let b = create_book(&s, book_input(Some(300))).await.unwrap();
        let r = start_reading(
            &s,
            b.id,
            StartReading {
                unit: "pages".into(),
            },
        )
        .await
        .unwrap();
        // Timestamped entries from the API or old backups coexist with UI dates.
        sqlx::query("INSERT INTO progress_entries (reading_id,value,recorded_at) VALUES (?,20,'2026-10-01 12:00:00')")
            .bind(r.id).execute(&s.db).await.unwrap();
        let last = add_progress(
            &s,
            r.id,
            ProgressInput {
                value: 30,
                note: None,
                recorded_at: Some("2026-10-01".into()),
            },
        )
        .await
        .unwrap();
        let _ = add_progress(
            &s,
            r.id,
            ProgressInput {
                value: 10,
                note: None,
                recorded_at: Some("2026-09-30".into()),
            },
        )
        .await
        .unwrap();
        assert_eq!(reading(&s.db, r.id).await.unwrap().current_value, 30);
        let detail = get_book(&s, b.id).await.unwrap();
        assert_eq!(detail["readings"][0]["progress"][0]["id"], last.id);
        let summary = stats(&s).await.unwrap();
        assert_eq!(summary["pages_read"], 30);
        assert_eq!(summary["daily_pages"][1]["pages"], 20);
        let _ = delete_progress(&s, last.id).await.unwrap();
        assert_eq!(reading(&s.db, r.id).await.unwrap().current_value, 20);
    }

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

    #[tokio::test]
    async fn native_database_survives_restart_and_backup_roundtrip() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("booklib.sqlite");
        let s = AppState::open(&path).await.unwrap();
        let b = create_book(&s, book_input(Some(300))).await.unwrap();
        let r = start_reading(
            &s,
            b.id,
            StartReading {
                unit: "pages".into(),
            },
        )
        .await
        .unwrap();
        add_progress(
            &s,
            r.id,
            ProgressInput {
                value: 42,
                note: Some("Local note".into()),
                recorded_at: Some("2026-10-01".into()),
            },
        )
        .await
        .unwrap();
        let backup = export_data(&s).await.unwrap();
        s.db.close().await;

        let reopened = AppState::open(&path).await.unwrap();
        assert_eq!(export_data(&reopened).await.unwrap(), backup);
        delete_book(&reopened, b.id).await.unwrap();
        assert!(list_books(&reopened).await.unwrap().is_empty());
        import_data(&reopened, serde_json::from_value(backup.clone()).unwrap())
            .await
            .unwrap();
        assert_eq!(export_data(&reopened).await.unwrap(), backup);

        // Invalid imports must roll back deletion of the current library.
        let mut invalid = backup.clone();
        let duplicate = invalid["books"][0].clone();
        invalid["books"].as_array_mut().unwrap().push(duplicate);
        assert!(
            import_data(&reopened, serde_json::from_value(invalid).unwrap())
                .await
                .is_err()
        );
        assert_eq!(export_data(&reopened).await.unwrap(), backup);
        reopened.db.close().await;
    }
}
