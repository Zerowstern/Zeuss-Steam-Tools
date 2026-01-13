// Download logic module
use std::process::{Command, Stdio};
use std::fs;
use std::path::Path;
use std::io::{BufRead, BufReader};
use regex::Regex;
use tauri::Emitter;
use serde::{Deserialize, Serialize};

use crate::steam::fetch_game_name;
use crate::manifest::parse_manifest;

pub fn emit_log(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("download-log", msg);
}

pub fn prepare_temp_dir() -> Result<std::path::PathBuf, String> {
    let temp_dir = Path::new("temp_depot_data");
    if temp_dir.exists() {
        fs::remove_dir_all(temp_dir).map_err(|e| format!("Clean temp dir failed: {}", e))?;
    }
    fs::create_dir_all(temp_dir).map_err(|e| format!("Create temp dir failed: {}", e))?;
    Ok(temp_dir.to_path_buf())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepotInfo {
    pub depot_id: String,
    pub manifest_id: String,
    pub file_path: String,
    pub file_count: u64,
    pub total_size: u64,
    pub os: String,
}

/// Prepare staging area and return list of available depots for user selection
pub fn prepare_morrenus_staging(app_id: &str, api_key: &str) -> Result<(std::path::PathBuf, Vec<DepotInfo>), String> {
    let temp_dir = prepare_temp_dir()?;

    let url = format!("https://manifest.morrenus.xyz/api/v1/manifest/{}", app_id);
    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url)
        .header("User-Agent", "GreenLumaManager/1.0")
        .header("X-API-Key", api_key)
        .send()
        .map_err(|e| format!("Request failed: {}", e))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("API Error: {}", status));
    }

    let zip_path = temp_dir.join("data.zip");
    {
        let mut file = fs::File::create(&zip_path).map_err(|e| e.to_string())?;
        let bytes = resp.bytes().map_err(|e| e.to_string())?;
        std::io::copy(&mut bytes.as_ref(), &mut file).map_err(|e| e.to_string())?;
    }

    {
        let file = fs::File::open(&zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Zip error: {}", e))?;
        archive.extract(&temp_dir).map_err(|e| format!("Extract error: {}", e))?;
    }

    let depots = scan_for_depots(&temp_dir)?;
    Ok((temp_dir, depots))
}

/// Prepare manual staging area and return list of available depots
pub fn prepare_manual_staging(source_path_str: &str) -> Result<(std::path::PathBuf, Vec<DepotInfo>), String> {
    let temp_dir = prepare_temp_dir()?;
    let source_path = Path::new(source_path_str);

    if !source_path.exists() {
        return Err(format!("Source path does not exist: {}", source_path_str));
    }

    if source_path.is_dir() {
        let paths = fs::read_dir(source_path).map_err(|e| e.to_string())?;
        for entry in paths {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    let dest = temp_dir.join(name);
                    fs::copy(&path, &dest).map_err(|e| e.to_string())?;
                }
            }
        }
    } else {
        let file = fs::File::open(source_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("Invalid Zip file: {}", e))?;
        archive.extract(&temp_dir).map_err(|e| format!("Extract error: {}", e))?;
    }

    let depots = scan_for_depots(&temp_dir)?;
    Ok((temp_dir, depots))
}

/// Scan staging directory for manifest files and extract depot info
fn scan_for_depots(staging_dir: &Path) -> Result<Vec<DepotInfo>, String> {
    let manifest_regex = Regex::new(r"(\d+)_(\d+)\.manifest$").unwrap();
    let mut depots = Vec::new();

    let paths = fs::read_dir(staging_dir).map_err(|e| e.to_string())?;
    for path in paths {
        let path = path.map_err(|e| e.to_string())?.path();
        if let Some(ext) = path.extension() {
            if ext == "manifest" {
                let file_name = path.file_name().unwrap().to_string_lossy();
                if let Some(caps) = manifest_regex.captures(&file_name) {
                    // Start parsing manifest content for info
                    let mut file_count = 0;
                    let mut total_size = 0;
                    let mut os = "Unknown".to_string();

                    if let Ok(summary) = parse_manifest(&path) {
                        file_count = summary.file_count;
                        total_size = summary.total_size;
                        os = summary.os;
                    }

                    depots.push(DepotInfo {
                        depot_id: caps[1].to_string(),
                        manifest_id: caps[2].to_string(),
                        file_path: path.to_string_lossy().to_string(),
                        file_count,
                        total_size,
                        os,
                    });
                }
            }
        }
    }

    Ok(depots)
}

/// Execute download for selected depots
pub fn execute_download(
    app: &tauri::AppHandle,
    app_id: &str,
    staging_dir: &Path,
    target_dir_str: &str,
    selected_depots: &[DepotInfo],
) -> Result<String, String> {
    let exe_path = find_depot_downloader().map_err(|e| {
        emit_log(app, &e);
        e
    })?;
    emit_log(app, &format!("Found Exe at: {:?}", exe_path));

    emit_log(app, "Fetching game name...");
    let game_name = fetch_game_name(app_id);
    let sanitized_name = game_name.replace(|c: char| !c.is_alphanumeric() && c != ' ', "").trim().to_string();
    emit_log(app, &format!("Game Name: {}", game_name));

    let base_dir = if target_dir_str.is_empty() {
        std::path::Path::new("downloads")
    } else {
        std::path::Path::new(target_dir_str)
    };

    let game_dir = base_dir.join(&sanitized_name);
    fs::create_dir_all(&game_dir).map_err(|e| format!("Could not create output dir: {}", e))?;

    // Prepare keys
    emit_log(app, "Parsing keys...");
    let keys_content = parse_lua_keys(staging_dir)?;
    
    if !keys_content.is_empty() {
        let keys_path = staging_dir.join("keys.txt");
        fs::write(&keys_path, keys_content).map_err(|e| e.to_string())?;
    } else {
        emit_log(app, "Warning: No keys found in lua files.");
    }

    if selected_depots.is_empty() {
        emit_log(app, "No depots selected.");
        return Err("No depots selected.".into());
    }

    let use_subdirs = selected_depots.len() > 1;
    emit_log(app, &format!("Download structure: {}", if use_subdirs { "Per-Depot Subdirectories" } else { "Single Directory" }));

    let keys_path_abs = staging_dir.join("keys.txt");
    let keys_arg = keys_path_abs.to_string_lossy();

    for depot in selected_depots {
        // Determine output directory based on depot count
        let output_dir = if use_subdirs {
            game_dir.join(&depot.depot_id)
        } else {
            game_dir.clone()
        };
        
        fs::create_dir_all(&output_dir).map_err(|e| format!("Could not create depot dir: {}", e))?;
        
        let output_dir_str = output_dir.to_string_lossy();
        emit_log(app, &format!("Processing Depot {} -> {:?}", depot.depot_id, output_dir));

        let mut child = Command::new(&exe_path)
            .args(&[
                "-app", app_id,
                "-depot", &depot.depot_id,
                "-manifest", &depot.manifest_id,
                "-manifestfile", &depot.file_path,
                "-depotkeys", &keys_arg,
                "-dir", &output_dir_str,
                "-max-downloads", "16"
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;

        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(l) = line {
                    emit_log(app, &l);
                }
            }
        }
        
        let status = child.wait().map_err(|e| e.to_string())?;

        if !status.success() {
            let err = format!("DepotDownloader failed for depot {}", depot.depot_id);
            emit_log(app, &err);
            return Err(err);
        }
    }

    if staging_dir.exists() {
        fs::remove_dir_all(staging_dir).ok();
    }
    
    emit_log(app, "Download Complete!");
    Ok(format!("Download complete for {}", app_id))
}

fn parse_lua_keys(staging_dir: &Path) -> Result<String, String> {
    let mut keys_content = String::new();
    let lua_pattern_1 = Regex::new(r#"addappid\((\d+),\s*\d+,\s*"([A-Za-z0-9]+)"\)"#).unwrap();
    let lua_pattern_2 = Regex::new(r#"\[(\d+)\]\s*=\s*"([A-Za-z0-9]+)""#).unwrap();

    let paths = fs::read_dir(staging_dir).map_err(|e| e.to_string())?;
    for path in paths {
        let path = path.map_err(|e| e.to_string())?.path();
        if let Some(ext) = path.extension() {
            if ext == "lua" {
                let content = fs::read_to_string(&path).unwrap_or_default();
                for cap in lua_pattern_1.captures_iter(&content) {
                    keys_content.push_str(&format!("{};{}\n", &cap[1], &cap[2]));
                }
                for cap in lua_pattern_2.captures_iter(&content) {
                    keys_content.push_str(&format!("{};{}\n", &cap[1], &cap[2]));
                }
            }
        }
    }
    Ok(keys_content)
}

fn find_depot_downloader() -> Result<std::path::PathBuf, String> {
    let exe_path = std::path::PathBuf::from("DepotDownloaderMod.exe");
    if exe_path.exists() {
        return Ok(exe_path);
    }
    
    let candidates = vec![
        "DepotDownloaderMod/net9.0/DepotDownloaderMod.exe",
        "../DepotDownloaderMod/net9.0/DepotDownloaderMod.exe",
        "../DepotDownloaderMod.exe"
    ];

    for c in candidates {
        let p = std::path::Path::new(c);
        if p.exists() {
            return Ok(p.to_path_buf());
        }
    }

    let cwd = std::env::current_dir().unwrap_or_default();
    Err(format!("DepotDownloaderMod.exe not found. Searched from: {:?}", cwd))
}

// Legacy functions for backwards compatibility (will be removed later)
pub fn manual_download_logic(app: &tauri::AppHandle, app_id: &str, source_path_str: &str, target_dir_str: &str) -> Result<String, String> {
    emit_log(app, "Starting Manual Process...");
    let (staging_dir, depots) = prepare_manual_staging(source_path_str)?;
    emit_log(app, &format!("Found {} depot(s)", depots.len()));
    execute_download(app, app_id, &staging_dir, target_dir_str, &depots)
}

pub fn download_logic(app: &tauri::AppHandle, app_id: &str, api_key: &str, target_dir_str: &str) -> Result<String, String> {
    emit_log(app, "Downloading manifest from Morrenus...");
    let (staging_dir, depots) = prepare_morrenus_staging(app_id, api_key)?;
    emit_log(app, &format!("Found {} depot(s)", depots.len()));
    execute_download(app, app_id, &staging_dir, target_dir_str, &depots)
}
