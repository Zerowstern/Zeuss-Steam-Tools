// Tauri command handlers
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

use crate::appid::{db_exists, search_games as appid_search, update_db};
use crate::download::{
    download_logic, execute_download, manual_download_logic, prepare_manual_staging,
    prepare_morrenus_staging, DepotInfo,
};
use crate::steam::GameInfo;

/// Staging state to hold prepared download data between commands
#[derive(Default)]
pub struct StagingState {
    pub staging_dir: Option<std::path::PathBuf>,
    pub app_id: String,
    pub target_dir: String,
    pub depots: Vec<DepotInfo>,
}

pub struct AppState(pub Mutex<StagingState>);

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}!", name)
}

#[tauri::command]
pub fn check_dotnet() -> bool {
    let output = std::process::Command::new("dotnet")
        .arg("--list-runtimes")
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains("Microsoft.NETCore.App 9.")
        }
        Err(_) => false,
    }
}

#[tauri::command]
pub async fn download_process(
    app: tauri::AppHandle,
    app_id: String,
    api_key: String,
    target_dir: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || download_logic(&app, &app_id, &api_key, &target_dir))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn manual_download_process(
    app: tauri::AppHandle,
    app_id: String,
    source_path: String,
    target_dir: String,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        manual_download_logic(&app, &app_id, &source_path, &target_dir)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn get_game_info(app_id: String) -> GameInfo {
    GameInfo::from_appid(&app_id)
}

#[derive(Serialize, Deserialize)]
pub struct GameSearchResult {
    pub appid: String,
    pub name: String,
    pub header_url: String,
}

#[tauri::command]
pub fn search_games(query: String) -> Vec<GameSearchResult> {
    match appid_search(&query, 20) {
        Ok(results) => results
            .into_iter()
            .map(|r| GameSearchResult {
                appid: r.appid,
                name: r.name,
                header_url: r.header_url,
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

#[tauri::command]
pub async fn update_appid_db() -> Result<(), String> {
    tokio::task::spawn_blocking(move || update_db().map(|_| ()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn check_appid_db() -> bool {
    db_exists()
}

// --- New Depot Selection Commands ---

#[tauri::command]
pub async fn prepare_morrenus_download(
    state: tauri::State<'_, AppState>,
    app_id: String,
    api_key: String,
    target_dir: String,
) -> Result<Vec<DepotInfo>, String> {
    let (staging_dir, depots) = tokio::task::spawn_blocking({
        let app_id = app_id.clone();
        let api_key = api_key.clone();
        move || prepare_morrenus_staging(&app_id, &api_key)
    })
    .await
    .map_err(|e| e.to_string())??;

    // Store in state
    let mut staging = state.0.lock().map_err(|e| e.to_string())?;
    staging.staging_dir = Some(staging_dir);
    staging.app_id = app_id;
    staging.target_dir = target_dir;
    staging.depots = depots.clone();

    Ok(depots)
}

#[tauri::command]
pub async fn prepare_manual_download(
    state: tauri::State<'_, AppState>,
    app_id: String,
    source_path: String,
    target_dir: String,
) -> Result<Vec<DepotInfo>, String> {
    let (staging_dir, depots) = tokio::task::spawn_blocking({
        let source_path = source_path.clone();
        move || prepare_manual_staging(&source_path)
    })
    .await
    .map_err(|e| e.to_string())??;

    // Store in state
    let mut staging = state.0.lock().map_err(|e| e.to_string())?;
    staging.staging_dir = Some(staging_dir);
    staging.app_id = app_id;
    staging.target_dir = target_dir;
    staging.depots = depots.clone();

    Ok(depots)
}

#[tauri::command]
pub async fn execute_selected_download(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    selected_depot_ids: Vec<String>,
) -> Result<String, String> {
    let (staging_dir, app_id, target_dir, all_depots) = {
        let staging = state.0.lock().map_err(|e| e.to_string())?;
        let dir = staging
            .staging_dir
            .clone()
            .ok_or("No staging directory. Call prepare first.")?;
        (
            dir,
            staging.app_id.clone(),
            staging.target_dir.clone(),
            staging.depots.clone(),
        )
    };

    // Filter to selected depots
    let selected_depots: Vec<DepotInfo> = all_depots
        .into_iter()
        .filter(|d| selected_depot_ids.contains(&d.depot_id))
        .collect();

    let result = tokio::task::spawn_blocking(move || {
        execute_download(&app, &app_id, &staging_dir, &target_dir, &selected_depots)
    })
    .await
    .map_err(|e| e.to_string())??;

    // Clear state
    let mut staging = state.0.lock().map_err(|e| e.to_string())?;
    *staging = StagingState::default();

    Ok(result)
}

#[tauri::command]
pub fn cancel_staging(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let mut staging = state.0.lock().map_err(|e| e.to_string())?;

    // Cleanup staging directory if exists
    if let Some(ref dir) = staging.staging_dir {
        if dir.exists() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    *staging = StagingState::default();
    Ok(())
}

#[tauri::command]
pub fn open_explorer_folder(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // Normalize path separators to backslashes for Windows Explorer
        let clean_path = path.replace("/", "\\");
        std::process::Command::new("explorer")
            .arg(clean_path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    // Fallback or other OS could go here if needed
    Ok(())
}
