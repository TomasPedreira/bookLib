use booklib::{catalog, library, AppState, ServiceResult};
use serde_json::Value;
use tauri::{Manager, State};

#[tauri::command]
async fn list_books(state: State<'_, AppState>) -> ServiceResult<Vec<Value>> {
    library::list_books(state.inner()).await
}

#[tauri::command]
async fn get_book(state: State<'_, AppState>, id: i64) -> ServiceResult<Value> {
    library::get_book(state.inner(), id).await
}

#[tauri::command]
async fn create_book(
    state: State<'_, AppState>,
    b: library::BookInput,
) -> ServiceResult<library::Book> {
    library::create_book(state.inner(), b).await
}

#[tauri::command]
async fn update_book(
    state: State<'_, AppState>,
    id: i64,
    b: library::BookInput,
) -> ServiceResult<library::Book> {
    library::update_book(state.inner(), id, b).await
}

#[tauri::command]
async fn update_rating(
    state: State<'_, AppState>,
    id: i64,
    input: library::RatingInput,
) -> ServiceResult<library::Book> {
    library::update_rating(state.inner(), id, input).await
}

#[tauri::command]
async fn delete_book(state: State<'_, AppState>, id: i64) -> ServiceResult<Value> {
    library::delete_book(state.inner(), id).await
}

#[tauri::command]
async fn start_reading(
    state: State<'_, AppState>,
    id: i64,
    input: library::StartReading,
) -> ServiceResult<library::Reading> {
    library::start_reading(state.inner(), id, input).await
}

#[tauri::command]
async fn delete_reading(state: State<'_, AppState>, id: i64) -> ServiceResult<Value> {
    library::delete_reading(state.inner(), id).await
}

#[tauri::command]
async fn change_reading(
    state: State<'_, AppState>,
    id: i64,
    input: library::ReadingStatus,
) -> ServiceResult<library::Reading> {
    library::change_reading(state.inner(), id, input).await
}

#[tauri::command]
async fn add_progress(
    state: State<'_, AppState>,
    id: i64,
    input: library::ProgressInput,
) -> ServiceResult<library::Progress> {
    library::add_progress(state.inner(), id, input).await
}

#[tauri::command]
async fn edit_progress(
    state: State<'_, AppState>,
    id: i64,
    input: library::ProgressInput,
) -> ServiceResult<library::Progress> {
    library::edit_progress(state.inner(), id, input).await
}

#[tauri::command]
async fn edit_progress_day(state: State<'_, AppState>, id: i64, input: library::ProgressInput) -> ServiceResult<library::Progress> {
    library::edit_progress_day(state.inner(), id, input).await
}

#[tauri::command]
async fn delete_progress(state: State<'_, AppState>, id: i64) -> ServiceResult<Value> {
    library::delete_progress(state.inner(), id).await
}

#[tauri::command]
async fn stats(state: State<'_, AppState>) -> ServiceResult<Value> {
    library::stats(state.inner()).await
}

#[tauri::command]
async fn export_data(state: State<'_, AppState>) -> ServiceResult<Value> {
    library::export_data(state.inner()).await
}

#[tauri::command]
async fn import_data(state: State<'_, AppState>, data: library::Backup) -> ServiceResult<Value> {
    library::import_data(state.inner(), data).await
}

#[tauri::command]
async fn work_genres(state: State<'_, AppState>, id: String) -> ServiceResult<Value> {
    catalog::work_genres(state.inner(), id).await
}

#[tauri::command]
async fn work_details(state: State<'_, AppState>, id: String) -> ServiceResult<Value> {
    catalog::work_details(state.inner(), id).await
}

#[tauri::command]
async fn search(state: State<'_, AppState>, query: catalog::SearchQuery) -> ServiceResult<Value> {
    catalog::search(state.inner(), query).await
}

#[tauri::command]
async fn lookup_isbn(state: State<'_, AppState>, input: String) -> ServiceResult<Value> {
    catalog::lookup_isbn(state.inner(), input).await
}

#[tauri::command]
async fn recommendations(
    state: State<'_, AppState>,
    query: catalog::RecommendationQuery,
) -> ServiceResult<Value> {
    catalog::recommendations(state.inner(), query).await
}

#[tauri::command]
async fn editions(
    state: State<'_, AppState>,
    id: String,
    query: catalog::EditionQuery,
) -> ServiceResult<Value> {
    catalog::editions(state.inner(), id, query).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            let state =
                tauri::async_runtime::block_on(AppState::open(&folder.join("booklib.sqlite")))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_books,
            get_book,
            create_book,
            update_book,
            update_rating,
            delete_book,
            start_reading,
            change_reading,
            delete_reading,
            add_progress,
            edit_progress,
            edit_progress_day,
            delete_progress,
            stats,
            export_data,
            import_data,
            work_genres,
            work_details,
            search,
            lookup_isbn,
            recommendations,
            editions
        ])
        .run(tauri::generate_context!())
        .expect("could not run BookLib");
}
