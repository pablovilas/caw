use crate::AppState;
use caw_core::NormalizedSession;

#[tauri::command]
pub async fn get_sessions(state: tauri::State<'_, AppState>) -> Result<Vec<NormalizedSession>, String> {
    Ok(state.monitor.snapshot().await)
}
