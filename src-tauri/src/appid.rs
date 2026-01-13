// App ID Database Manager - Search games by name
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const DB_FILE: &str = "games_appid.json";
const DB_URL: &str = "https://github.com/jsnli/steamappidlist/raw/refs/heads/master/data/games_appid.json";

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppEntry {
    pub appid: u64,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppListWrapper {
    applist: AppListInner,
}

#[derive(Debug, Serialize, Deserialize)]
struct AppListInner {
    apps: Vec<AppEntry>,
}

/// Check if the database file exists
pub fn db_exists() -> bool {
    Path::new(DB_FILE).exists()
}

/// Download/update the AppID database
pub fn update_db() -> Result<String, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(DB_URL)
        .send()
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("HTTP Error: {}", resp.status()));
    }

    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    fs::write(DB_FILE, &bytes).map_err(|e| format!("Write failed: {}", e))?;

    Ok("Database updated successfully".to_string())
}

/// Load and parse the database
fn load_db() -> Result<Vec<AppEntry>, String> {
    if !db_exists() {
        return Err("Database not found. Please update first.".to_string());
    }

    let content = fs::read_to_string(DB_FILE).map_err(|e| e.to_string())?;
    
    // Try parsing as wrapped format first
    if let Ok(wrapper) = serde_json::from_str::<AppListWrapper>(&content) {
        return Ok(wrapper.applist.apps);
    }
    
    // Try parsing as direct array
    if let Ok(apps) = serde_json::from_str::<Vec<AppEntry>>(&content) {
        return Ok(apps);
    }

    Err("Failed to parse database format".to_string())
}

#[derive(Serialize, Clone)]
pub struct SearchResult {
    pub appid: String,
    pub name: String,
    pub header_url: String,
}

/// Search for games by name
pub fn search_games(query: &str, limit: usize) -> Result<Vec<SearchResult>, String> {
    let apps = load_db()?;
    let query_lower = query.to_lowercase();

    let results: Vec<SearchResult> = apps
        .iter()
        .filter(|app| app.name.to_lowercase().contains(&query_lower))
        .take(limit)
        .map(|app| SearchResult {
            appid: app.appid.to_string(),
            name: app.name.clone(),
            header_url: format!(
                "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/{}/header.jpg",
                app.appid
            ),
        })
        .collect();

    Ok(results)
}
