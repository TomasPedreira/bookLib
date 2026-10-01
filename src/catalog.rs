use crate::{bad, AppError, AppState, ServiceResult};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
}
#[derive(Deserialize)]
pub struct EditionQuery {
    offset: Option<usize>,
}

#[derive(Deserialize)]
pub struct RecommendationQuery {
    genres: Option<String>,
    languages: Option<String>,
    seen: Option<String>,
    interests: Option<String>,
    avoid: Option<String>,
    mode: Option<String>,
}

const LANGUAGES: &[&str] = &["eng", "por", "spa", "fre", "ger", "ita"];
pub(crate) const TOPICS: &[&str] = &[
    "fiction",
    "fantasy",
    "science_fiction",
    "romance",
    "mystery",
    "thriller",
    "horror",
    "historical_fiction",
    "young_adult",
    "adventure",
    "poetry",
    "comics",
    "biography",
    "history",
    "philosophy",
    "psychology",
    "science",
    "technology",
    "business",
    "self_help",
    "health",
    "cooking",
    "art",
    "travel",
    "memoir",
    "autobiography",
    "politics",
    "religion",
    "classics",
    "crime",
    "dystopian",
    "short_stories",
    "literary_fiction",
    "humor",
    "nonfiction",
    "finance",
    "sports",
    "education",
    "music",
    "children",
    "graphic_novels",
    "manga",
];

fn valid_olid(s: &str, suffix: char) -> bool {
    let Some(mid) = s.strip_prefix("OL").and_then(|x| x.strip_suffix(suffix)) else {
        return false;
    };
    !mid.is_empty() && mid.len() <= 12 && mid.bytes().all(|b| b.is_ascii_digit())
}

fn normalize_isbn(input: &str) -> Option<String> {
    let isbn: String = input
        .chars()
        .filter(|c| *c != '-' && !c.is_whitespace())
        .collect::<String>()
        .to_ascii_uppercase();
    let bytes = isbn.as_bytes();
    let valid = match bytes.len() {
        10 => {
            let sum: u32 = bytes
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    let digit = if i == 9 && c == b'X' {
                        10
                    } else if c.is_ascii_digit() {
                        (c - b'0') as u32
                    } else {
                        100
                    };
                    digit * (10 - i as u32)
                })
                .sum();
            bytes[..9].iter().all(u8::is_ascii_digit)
                && (bytes[9].is_ascii_digit() || bytes[9] == b'X')
                && sum % 11 == 0
        }
        13 => {
            bytes.iter().all(u8::is_ascii_digit)
                && bytes
                    .iter()
                    .enumerate()
                    .map(|(i, c)| (c - b'0') as u32 * if i % 2 == 0 { 1 } else { 3 })
                    .sum::<u32>()
                    % 10
                    == 0
        }
        _ => false,
    };
    valid.then_some(isbn)
}

fn work_genre_names(work: &Value) -> Vec<&'static str> {
    let mut genres = Vec::new();
    for subject in work["subjects"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        let name = subject.to_lowercase();
        let genre = if name.contains("science fiction") || name.contains("sci-fi") {
            Some("science_fiction")
        } else if name.contains("historical fiction") {
            Some("historical_fiction")
        } else if name.contains("literary fiction") {
            Some("literary_fiction")
        } else if name.contains("graphic novel") {
            Some("graphic_novels")
        } else if name.contains("short stor") {
            Some("short_stories")
        } else if name.contains("autobiograph") {
            Some("autobiography")
        } else if name.contains("memoir") {
            Some("memoir")
        } else if name.contains("fantasy") {
            Some("fantasy")
        } else if name.contains("mystery") || name.contains("detective") {
            Some("mystery")
        } else if name.contains("dystopi") {
            Some("dystopian")
        } else if name.contains("horror") {
            Some("horror")
        } else if name.contains("romance") {
            Some("romance")
        } else if name.contains("biography") {
            Some("biography")
        } else if name.contains("manga") {
            Some("manga")
        } else if name.contains("politic") {
            Some("politics")
        } else if name.contains("business") {
            Some("business")
        } else if name.contains("finance") {
            Some("finance")
        } else if name == "history" {
            Some("history")
        } else {
            None
        };
        if let Some(genre) = genre {
            if !genres.contains(&genre) {
                genres.push(genre);
            }
        }
    }
    genres
}

pub(crate) async fn default_book_topics(s: &AppState, work_id: &str) -> Result<String, AppError> {
    if !valid_olid(work_id, 'W') {
        return Err(bad("Identificador de obra inválido"));
    }
    let url = format!("https://openlibrary.org/works/{work_id}.json");
    let work = fetch(s, &url, &[]).await?;
    Ok(work_genre_names(&work)
        .into_iter()
        .take(4)
        .collect::<Vec<_>>()
        .join(","))
}

pub async fn work_genres(s: &AppState, id: String) -> ServiceResult<Value> {
    if !valid_olid(&id, 'W') {
        return Err(bad("Identificador de obra inválido"));
    }
    let url = format!("https://openlibrary.org/works/{id}.json");
    let work = fetch(s, &url, &[]).await?;
    Ok(json!({"genres": work_genre_names(&work)}))
}

pub async fn work_details(s: &AppState, id: String) -> ServiceResult<Value> {
    if !valid_olid(&id, 'W') {
        return Err(bad("Identificador de obra inválido"));
    }
    let url = format!("https://openlibrary.org/works/{id}.json");
    let work = fetch(s, &url, &[]).await?;
    let description = work["description"]
        .as_str()
        .or_else(|| work["description"]["value"].as_str())
        .map(|text| text.trim().chars().take(5_000).collect::<String>())
        .filter(|text| !text.is_empty());
    Ok(json!({"description": description}))
}

async fn fetch(state: &AppState, url: &str, params: &[(&str, &str)]) -> Result<Value, AppError> {
    const CACHE_TTL: Duration = Duration::from_secs(1_800);
    const CACHE_LIMIT: usize = 256;
    let key = format!("{url}{}", serde_json::to_string(params).unwrap_or_default());
    if let Some((at, value)) = state.catalog_cache.lock().await.get(&key) {
        if at.elapsed() < CACHE_TTL {
            return Ok(value.clone());
        }
    }
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
        if response.status() == StatusCode::NOT_FOUND {
            return Err(AppError(
                StatusCode::NOT_FOUND,
                "Edição não encontrada no catálogo".into(),
            ));
        }
        return Err(AppError(
            StatusCode::BAD_GATEWAY,
            "O catálogo não respondeu à pesquisa".into(),
        ));
    }
    let value = response.json::<Value>().await.map_err(|_| {
        AppError(
            StatusCode::BAD_GATEWAY,
            "Resposta inválida do catálogo".into(),
        )
    })?;
    let mut cache = state.catalog_cache.lock().await;
    cache.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
    if cache.len() >= CACHE_LIMIT {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, (at, _))| *at)
            .map(|(key, _)| key.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(key, (Instant::now(), value.clone()));
    Ok(value)
}

pub async fn search(s: &AppState, query: SearchQuery) -> ServiceResult<Value> {
    let q = query.q.trim();
    if q.len() < 2 || q.len() > 120 {
        return Err(bad("Pesquisa entre 2 e 120 caracteres"));
    }
    let data = fetch(
        s,
        "https://openlibrary.org/search.json",
        &[
            ("q", q),
            ("limit", "16"),
            ("lang", "pt"),
            (
                "fields",
                "key,title,author_name,first_publish_year,cover_i,edition_count",
            ),
        ],
    )
    .await?;
    let items: Vec<Value> = data["docs"].as_array().into_iter().flatten().filter_map(|d| {
        let work_id = d["key"].as_str()?.trim_start_matches("/works/");
        if !valid_olid(work_id, 'W') { return None; }
        let cover_url = d["cover_i"].as_i64().map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg"));
        Some(json!({"work_id": work_id, "title": d["title"], "authors": d["author_name"],
            "year": d["first_publish_year"], "cover_url": cover_url, "edition_count": d["edition_count"]}))
    }).collect();
    Ok(
        json!({"items": items, "total": data["numFound"].as_i64().or_else(|| data["num_found"].as_i64()).unwrap_or(0)}),
    )
}

pub async fn lookup_isbn(s: &AppState, input: String) -> ServiceResult<Value> {
    let isbn = normalize_isbn(&input).ok_or_else(|| bad("ISBN inválido"))?;
    let url = format!("https://openlibrary.org/isbn/{isbn}.json");
    let edition = fetch(s, &url, &[]).await?;
    let edition_id = edition["key"]
        .as_str()
        .and_then(|key| key.strip_prefix("/books/"))
        .filter(|id| valid_olid(id, 'M'))
        .ok_or_else(|| bad("Edição inválida no catálogo"))?;
    let work_id = edition["works"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|work| work["key"].as_str()?.strip_prefix("/works/"))
        .find(|id| valid_olid(id, 'W'));
    let search_q = format!("isbn:{isbn}");
    let search = fetch(
        s,
        "https://openlibrary.org/search.json",
        &[
            ("q", &search_q),
            ("limit", "5"),
            ("fields", "key,title,author_name,cover_i"),
        ],
    )
    .await
    .ok();
    let matched = search
        .as_ref()
        .and_then(|data| data["docs"].as_array())
        .and_then(|docs| {
            docs.iter().find(|doc| {
                work_id.is_some_and(|id| doc["key"].as_str() == Some(&format!("/works/{id}")))
            })
        });
    let authors: Vec<&str> = matched
        .and_then(|doc| doc["author_name"].as_array())
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .take(3)
        .collect();
    let cover_id = edition["covers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_i64)
        .find(|id| *id > 0)
        .or_else(|| matched.and_then(|doc| doc["cover_i"].as_i64()));
    let cover_url = cover_id.map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg"));
    let language = edition["languages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["key"].as_str()?.strip_prefix("/languages/"))
        .next();
    Ok(json!({
        "edition_id": edition_id, "work_id": work_id, "isbn": isbn,
        "title": edition["title"], "authors": authors, "cover_url": cover_url,
        "page_count": edition["number_of_pages"], "published": edition["publish_date"],
        "language": language,
    }))
}

pub async fn recommendations(s: &AppState, query: RecommendationQuery) -> ServiceResult<Value> {
    let home = query.mode.as_deref() == Some("home");
    let chosen: Vec<&str> = query
        .genres
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|g| TOPICS.contains(g))
        .take(4)
        .collect();
    let seen: Vec<&str> = query
        .seen
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|id| valid_olid(id, 'W'))
        .take(30)
        .collect();
    let mut interests = std::collections::HashMap::<&'static str, usize>::new();
    if home {
        for pair in query.interests.as_deref().unwrap_or("").split(',') {
            let Some((genre, count)) = pair.split_once(':') else {
                continue;
            };
            if let (Some(genre), Ok(count)) = (
                TOPICS.iter().copied().find(|g| *g == genre),
                count.parse::<usize>(),
            ) {
                *interests.entry(genre).or_default() += count.min(50);
            }
        }
        if interests.is_empty() {
            for id in seen.iter().take(4) {
                let url = format!("https://openlibrary.org/works/{id}.json");
                if let Ok(work) = fetch(s, &url, &[]).await {
                    for genre in work_genre_names(&work) {
                        *interests.entry(genre).or_default() += 1;
                    }
                }
            }
        }
    }
    let mut ranked_interests: Vec<_> = interests.into_iter().collect();
    ranked_interests.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let avoided: Vec<&str> = query
        .avoid
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|genre| TOPICS.contains(genre) && !chosen.contains(genre))
        .collect();
    ranked_interests.retain(|(genre, _)| !avoided.contains(genre));
    let mut genres: Vec<&str> = ranked_interests
        .iter()
        .take(3)
        .map(|(genre, _)| *genre)
        .collect();
    for genre in chosen {
        if !genres.contains(&genre) && genres.len() < 4 {
            genres.push(genre);
        }
    }
    if home || genres.is_empty() {
        for genre in ["fiction", "fantasy", "science_fiction", "mystery"]
            .into_iter()
            .chain(TOPICS.iter().copied())
        {
            if genres.len() >= 4 {
                break;
            }
            if !genres.contains(&genre) && !avoided.contains(&genre) {
                genres.push(genre);
            }
        }
    }
    let languages: Vec<&str> = query
        .languages
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter(|l| LANGUAGES.contains(l))
        .collect();
    let owned: Vec<(Option<String>, String)> =
        sqlx::query_as("SELECT work_id, lower(trim(title)) FROM books")
            .fetch_all(&s.db)
            .await?;
    let mut candidates: std::collections::HashMap<String, (f64, Value)> =
        std::collections::HashMap::new();
    let mut requests = tokio::task::JoinSet::new();
    for (genre_index, genre) in genres.iter().enumerate() {
        let state = s.clone();
        let q = format!("subject_key:\"{genre}\"");
        requests.spawn(async move {
            let data = fetch(&state, "https://openlibrary.org/search.json", &[
                ("q", q.as_str()), ("sort", "trending"), ("limit", "50"),
                ("fields", "key,title,author_name,first_publish_year,cover_i,edition_count,language,ratings_average,ratings_count,readinglog_count"),
            ]).await;
            (genre_index, data)
        });
    }
    let mut results = Vec::new();
    let mut last_error = None;
    while let Some(result) = requests.join_next().await {
        let (index, data) = result.map_err(|error| {
            tracing::error!(%error, "recommendation task failed");
            AppError(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Erro nas recomendações".into(),
            )
        })?;
        match data {
            Ok(data) => results.push((index, data)),
            Err(error) => {
                tracing::warn!(genre = genres[index], "recommendation topic unavailable");
                last_error = Some(error);
            }
        }
    }
    if results.is_empty() {
        return Err(last_error.unwrap_or_else(|| bad("Sem recomendações disponíveis")));
    }
    results.sort_by_key(|(index, _)| *index);
    for (genre_index, data) in results {
        let genre = genres[genre_index];
        for (position, doc) in data["docs"].as_array().into_iter().flatten().enumerate() {
            let Some(work_id) = doc["key"].as_str().and_then(|k| k.strip_prefix("/works/")) else {
                continue;
            };
            if !valid_olid(work_id, 'W') {
                continue;
            }
            let Some(title) = doc["title"].as_str() else {
                continue;
            };
            let title_key = title.trim().to_lowercase();
            if owned
                .iter()
                .any(|(id, name)| id.as_deref() == Some(work_id) || name == &title_key)
                || (home && seen.contains(&work_id))
            {
                continue;
            }
            if !languages.is_empty()
                && !doc["language"].as_array().is_some_and(|available| {
                    available
                        .iter()
                        .any(|l| l.as_str().is_some_and(|code| languages.contains(&code)))
                })
            {
                continue;
            }
            let Some(cover_id) = doc["cover_i"].as_i64() else {
                continue;
            };
            let authors: Vec<&str> = doc["author_name"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .take(2)
                .collect();
            let count = doc["ratings_count"].as_i64().unwrap_or(0).max(0) as f64;
            let average = doc["ratings_average"]
                .as_f64()
                .unwrap_or(0.0)
                .clamp(0.0, 5.0);
            let adjusted_rating = if count > 0.0 {
                (average * count + 3.5 * 20.0) / (count + 20.0)
            } else {
                3.5
            };
            let log_count = (count + 1.0).ln();
            let score = adjusted_rating * 2.0
                + log_count * 0.55
                + (50 - position) as f64 * 0.025
                + if home && genre_index < ranked_interests.len().min(3) {
                    1.5 - genre_index as f64 * 0.3
                } else {
                    0.0
                };
            let item = json!({
                "work_id": work_id, "title": title, "authors": authors,
                "year": doc["first_publish_year"], "cover_url": format!("https://covers.openlibrary.org/b/id/{cover_id}-M.jpg"),
                "edition_count": doc["edition_count"], "genre": genre,
                "ratings_average": if count > 0.0 { Some(average) } else { None }, "ratings_count": count as i64,
            });
            let entry = candidates
                .entry(work_id.to_string())
                .or_insert((score, item.clone()));
            if score > entry.0 {
                *entry = (score, item);
            }
        }
    }
    let mut ranked: Vec<_> = candidates.into_values().collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    let limit: usize = if home { 10 } else { 24 };
    let per_genre = limit.div_ceil(genres.len());
    let mut counts = std::collections::HashMap::<String, usize>::new();
    let mut selected = Vec::new();
    let mut remainder = Vec::new();
    for (_, item) in ranked {
        let genre = item["genre"].as_str().unwrap_or("").to_string();
        let count = counts.entry(genre).or_default();
        if *count < per_genre && selected.len() < limit {
            *count += 1;
            selected.push(item);
        } else {
            remainder.push(item);
        }
    }
    selected.extend(remainder.into_iter().take(limit - selected.len()));
    Ok(json!({"items": selected}))
}

pub async fn editions(s: &AppState, id: String, query: EditionQuery) -> ServiceResult<Value> {
    if !valid_olid(&id, 'W') {
        return Err(bad("Identificador de obra inválido"));
    }
    let offset = query.offset.unwrap_or(0).min(1000).to_string();
    let url = format!("https://openlibrary.org/works/{id}/editions.json");
    let data = fetch(s, &url, &[("limit", "50"), ("offset", &offset)]).await?;
    let items: Vec<Value> = data["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let edition_id = e["key"].as_str()?.trim_start_matches("/books/");
            if !valid_olid(edition_id, 'M') {
                return None;
            }
            let cover_url = e["covers"]
                .as_array()
                .and_then(|v| v.first())
                .and_then(Value::as_i64)
                .map(|id| format!("https://covers.openlibrary.org/b/id/{id}-M.jpg"));
            let isbn = e["isbn_13"]
                .as_array()
                .and_then(|v| v.first())
                .or_else(|| e["isbn_10"].as_array().and_then(|v| v.first()));
            let language = e["languages"]
                .as_array()
                .and_then(|v| v.first())
                .and_then(|v| v["key"].as_str())
                .map(|v| v.trim_start_matches("/languages/"));
            Some(
                json!({"edition_id": edition_id, "work_id": id, "title": e["title"],
            "isbn": isbn, "page_count": e["number_of_pages"], "published": e["publish_date"],
            "language": language, "cover_url": cover_url}),
            )
        })
        .collect();
    Ok(
        json!({"items": items, "total": data["size"].as_i64().unwrap_or(0), "offset": offset.parse::<usize>().unwrap_or(0)}),
    )
}

#[cfg(test)]
mod tests {
    use super::{normalize_isbn, work_genre_names};
    use serde_json::json;

    #[test]
    fn accepts_isbn_10_and_13_with_separators() {
        assert_eq!(normalize_isbn("0-14-032872-6"), Some("0140328726".into()));
        assert_eq!(
            normalize_isbn("978 0 14 032872 1"),
            Some("9780140328721".into())
        );
        assert_eq!(normalize_isbn("0-8044-2957-X"), Some("080442957X".into()));
        assert_eq!(normalize_isbn("9780140328722"), None);
        assert_eq!(normalize_isbn("hello"), None);
    }

    #[test]
    fn recognises_life_writing_subjects_for_recommendations() {
        let work = json!({"subjects":["Biography", "Personal memoirs", "Business"]});
        assert_eq!(
            work_genre_names(&work),
            vec!["biography", "memoir", "business"]
        );
    }
}
