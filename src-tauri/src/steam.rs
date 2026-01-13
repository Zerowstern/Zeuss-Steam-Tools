// Utility functions for Steam API interaction
use serde::{Deserialize, Serialize};

/// Fetch game name from Steam API
pub fn fetch_game_name(app_id: &str) -> String {
    let url = format!("https://store.steampowered.com/api/appdetails?appids={}", app_id);
    if let Ok(resp) = reqwest::blocking::get(&url) {
        if let Ok(text) = resp.text() {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(app_data) = json.get(app_id) {
                    if let Some(data) = app_data.get("data") {
                        if let Some(name) = data.get("name") {
                            return name.as_str().unwrap_or(&format!("AppID_{}", app_id)).to_string();
                        }
                    }
                }
            }
        }
    }
    format!("AppID_{}", app_id)
}

/// Get the header image URL for a Steam app
pub fn get_header_image_url(app_id: &str) -> String {
    format!("https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/{}/header.jpg", app_id)
}

#[derive(Serialize, Deserialize, Clone)]
pub struct GameInfo {
    pub appid: String,
    pub name: String,
    pub header_url: String,
}

impl GameInfo {
    pub fn from_appid(app_id: &str) -> Self {
        let name = fetch_game_name(app_id);
        GameInfo {
            appid: app_id.to_string(),
            name,
            header_url: get_header_image_url(app_id),
        }
    }
}
