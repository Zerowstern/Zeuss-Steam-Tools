// Goldberg Steam Emulator (GBE Fork) integration
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::Emitter;

/// Goldberg Emulator configuration options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldbergConfig {
    // User settings
    pub account_name: String,
    pub steam_id: String,
    pub language: String,
    pub listen_port: u32,
    pub custom_broadcast_ip: Option<String>,

    // Feature toggles
    pub disable_networking: bool,
    pub offline_mode: bool,
    pub enable_overlay: bool,

    // Application settings
    pub use_experimental: bool,
    pub custom_save_location: Option<String>,
    pub generate_interfaces: bool,
    pub force_generate_all: bool,
}

impl Default for GoldbergConfig {
    fn default() -> Self {
        Self {
            account_name: "Goldberg".to_string(),
            steam_id: "76561197960287930".to_string(),
            language: "english".to_string(),
            listen_port: 47584,
            custom_broadcast_ip: None,
            disable_networking: false,
            offline_mode: false,
            enable_overlay: false,
            use_experimental: false,
            custom_save_location: None,
            generate_interfaces: false,
            force_generate_all: false,
        }
    }
}

/// Result of applying emulator to a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoldbergApplyResult {
    pub success: bool,
    pub message: String,
    pub steam_api_files: Vec<String>,
    pub config_path: Option<String>,
    pub backups_created: Vec<String>,
}

fn emit_log(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("goldberg-log", msg);
}

/// Available languages for Goldberg
pub const LANGUAGES: &[&str] = &[
    "english",
    "arabic",
    "bulgarian",
    "schinese",
    "tchinese",
    "czech",
    "danish",
    "dutch",
    "finnish",
    "french",
    "german",
    "greek",
    "hungarian",
    "indonesian",
    "italian",
    "japanese",
    "koreana",
    "latam",
    "norwegian",
    "polish",
    "portuguese",
    "brazilian",
    "romanian",
    "russian",
    "spanish",
    "swedish",
    "thai",
    "turkish",
    "ukrainian",
    "vietnamese",
];

/// Find the GBE emulator base path
fn find_emulator_path() -> Result<PathBuf, String> {
    let candidates = vec![
        "emu-win-release/release",
        "../emu-win-release/release",
        "gbe_fork/release",
    ];

    for c in &candidates {
        let p = Path::new(c);
        if p.exists() && p.is_dir() {
            return Ok(p.to_path_buf());
        }
    }

    // Check relative to exe
    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            for c in &candidates {
                let p = parent.join(c);
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    Err("GBE Emulator not found. Make sure emu-win-release/release folder exists.".to_string())
}

/// Scan a game folder for steam_api DLLs
pub fn scan_steam_api_dlls(game_folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dlls = Vec::new();

    if !game_folder.exists() {
        return Err(format!("Game folder not found: {:?}", game_folder));
    }

    fn scan_recursive(dir: &Path, dlls: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_dir() {
                scan_recursive(&path, dlls)?;
            } else if path.is_file() {
                if let Some(name) = path.file_name() {
                    let name_lower = name.to_string_lossy().to_lowercase();
                    if name_lower == "steam_api.dll" || name_lower == "steam_api64.dll" {
                        dlls.push(path);
                    }
                }
            }
        }
        Ok(())
    }

    scan_recursive(game_folder, &mut dlls)?;
    Ok(dlls)
}

/// Create backup of original files
fn backup_file(file_path: &Path) -> Result<PathBuf, String> {
    let backup_path = file_path.with_extension("dll.original");

    // Don't overwrite existing backup
    if !backup_path.exists() {
        fs::copy(file_path, &backup_path)
            .map_err(|e| format!("Failed to backup {:?}: {}", file_path, e))?;
    }

    Ok(backup_path)
}

/// Generate steam_settings folder with configuration
fn create_steam_settings(
    target_folder: &Path,
    app_id: &str,
    config: &GoldbergConfig,
) -> Result<PathBuf, String> {
    let settings_dir = target_folder.join("steam_settings");
    fs::create_dir_all(&settings_dir).map_err(|e| e.to_string())?;

    // Write steam_appid.txt
    let appid_path = settings_dir.join("steam_appid.txt");
    fs::write(&appid_path, app_id).map_err(|e| e.to_string())?;

    // Write configs.user.ini
    let user_config = format!(
        r#"[user::general]
account_name={}
account_steamid={}
language={}
ip_country=US

[user::saves]
{}
"#,
        config.account_name,
        config.steam_id,
        config.language,
        if let Some(ref save_path) = config.custom_save_location {
            format!("local_save_path={}", save_path)
        } else {
            "# local_save_path=".to_string()
        }
    );
    fs::write(settings_dir.join("configs.user.ini"), user_config).map_err(|e| e.to_string())?;

    // Write configs.main.ini
    let main_config = format!(
        r#"[main::connectivity]
disable_networking={}
listen_port={}
offline={}
{}

[main::general]
enable_account_avatar=0
"#,
        if config.disable_networking { 1 } else { 0 },
        config.listen_port,
        if config.offline_mode { 1 } else { 0 },
        if let Some(ref ip) = config.custom_broadcast_ip {
            format!("# Custom broadcast will use custom_broadcasts.txt")
        } else {
            String::new()
        }
    );
    fs::write(settings_dir.join("configs.main.ini"), main_config).map_err(|e| e.to_string())?;

    // Write custom_broadcasts.txt if specified
    if let Some(ref ip) = config.custom_broadcast_ip {
        fs::write(settings_dir.join("custom_broadcasts.txt"), ip).map_err(|e| e.to_string())?;
    }

    // Write configs.overlay.ini if overlay is enabled
    if config.enable_overlay {
        let overlay_config = r#"[overlay::general]
enable_experimental_overlay=1
"#;
        fs::write(settings_dir.join("configs.overlay.ini"), overlay_config)
            .map_err(|e| e.to_string())?;
    }

    Ok(settings_dir)
}

/// Detect if a DLL is 32-bit or 64-bit
fn detect_dll_architecture(dll_path: &Path) -> Result<String, String> {
    // Read PE header to determine architecture
    let data = fs::read(dll_path).map_err(|e| e.to_string())?;

    if data.len() < 64 {
        return Err("File too small to be a valid DLL".to_string());
    }

    // Check MZ header
    if data[0] != b'M' || data[1] != b'Z' {
        return Err("Not a valid PE file".to_string());
    }

    // Get PE header offset from 0x3C
    let pe_offset = u32::from_le_bytes([data[0x3C], data[0x3D], data[0x3E], data[0x3F]]) as usize;

    if data.len() < pe_offset + 6 {
        return Err("Invalid PE offset".to_string());
    }

    // Check PE signature
    if data[pe_offset] != b'P' || data[pe_offset + 1] != b'E' {
        return Err("Invalid PE signature".to_string());
    }

    // Machine type is at PE offset + 4
    let machine = u16::from_le_bytes([data[pe_offset + 4], data[pe_offset + 5]]);

    match machine {
        0x014c => Ok("x32".to_string()), // IMAGE_FILE_MACHINE_I386
        0x8664 => Ok("x64".to_string()), // IMAGE_FILE_MACHINE_AMD64
        _ => Err(format!("Unknown architecture: 0x{:04X}", machine)),
    }
}

/// Generate steam_interfaces.txt from original DLL
fn generate_interfaces_file(
    app: &tauri::AppHandle,
    original_dll: &Path,
    target_folder: &Path,
    emu_path: &Path,
) -> Result<(), String> {
    let arch = detect_dll_architecture(original_dll)?;

    let generator_name = if arch == "x64" {
        "generate_interfaces_x64.exe"
    } else {
        "generate_interfaces_x32.exe"
    };

    let generator_path = emu_path
        .join("tools")
        .join("generate_interfaces")
        .join(generator_name);

    if !generator_path.exists() {
        emit_log(
            app,
            &format!("Interface generator not found: {:?}", generator_path),
        );
        return Err("Interface generator not found".to_string());
    }

    // We need to copy the original DLL to a temp location with a predictable name
    let temp_dll = target_folder.join("steam_api_temp.dll");
    fs::copy(original_dll, &temp_dll).map_err(|e| e.to_string())?;

    emit_log(app, "Generating steam_interfaces.txt...");

    let output = Command::new(&generator_path)
        .arg(&temp_dll)
        .current_dir(target_folder)
        .output()
        .map_err(|e| format!("Failed to run interface generator: {}", e))?;

    // Clean up temp DLL
    let _ = fs::remove_file(&temp_dll);

    // Check if interfaces file was created
    let interfaces_file = target_folder.join("steam_interfaces.txt");
    if interfaces_file.exists() {
        // Move to steam_settings
        let settings_dir = target_folder.join("steam_settings");
        fs::create_dir_all(&settings_dir).ok();
        let dest = settings_dir.join("steam_interfaces.txt");
        fs::rename(&interfaces_file, &dest).map_err(|e| e.to_string())?;
        emit_log(app, "  Generated steam_interfaces.txt");
        Ok(())
    } else {
        emit_log(
            app,
            "  Could not generate interfaces file (might not be needed)",
        );
        Ok(()) // Not fatal
    }
}

/// Apply Goldberg emulator to a game folder
pub fn apply_emulator(
    app: &tauri::AppHandle,
    game_folder: &Path,
    app_id: &str,
    config: &GoldbergConfig,
) -> GoldbergApplyResult {
    emit_log(
        app,
        &format!("Applying Goldberg Emulator to: {:?}", game_folder),
    );

    // Find emulator path
    let emu_path = match find_emulator_path() {
        Ok(p) => p,
        Err(e) => {
            return GoldbergApplyResult {
                success: false,
                message: e,
                steam_api_files: vec![],
                config_path: None,
                backups_created: vec![],
            };
        }
    };

    emit_log(app, &format!("Using emulator from: {:?}", emu_path));

    // Scan for steam_api DLLs
    let dlls = match scan_steam_api_dlls(game_folder) {
        Ok(d) => d,
        Err(e) => {
            return GoldbergApplyResult {
                success: false,
                message: e,
                steam_api_files: vec![],
                config_path: None,
                backups_created: vec![],
            };
        }
    };

    if dlls.is_empty() {
        return GoldbergApplyResult {
            success: false,
            message: "No steam_api.dll or steam_api64.dll found in game folder".to_string(),
            steam_api_files: vec![],
            config_path: None,
            backups_created: vec![],
        };
    }

    emit_log(app, &format!("Found {} Steam API DLL(s)", dlls.len()));

    let mut steam_api_files = Vec::new();
    let mut backups_created = Vec::new();
    let mut errors = Vec::new();

    for dll_path in &dlls {
        let dll_name = dll_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        let dll_folder = dll_path.parent().unwrap();

        emit_log(app, &format!("\nProcessing: {:?}", dll_path));

        // Detect architecture
        let arch = match detect_dll_architecture(dll_path) {
            Ok(a) => a,
            Err(e) => {
                emit_log(app, &format!("  Error detecting architecture: {}", e));
                errors.push(e);
                continue;
            }
        };

        emit_log(app, &format!("  Architecture: {}", arch));

        // Create backup
        match backup_file(dll_path) {
            Ok(backup_path) => {
                emit_log(app, &format!("  Backup created: {:?}", backup_path));
                backups_created.push(backup_path.to_string_lossy().to_string());
            }
            Err(e) => {
                emit_log(app, &format!("  Warning: Could not create backup: {}", e));
            }
        }

        // Determine source DLL path
        let variant = if config.use_experimental {
            "experimental"
        } else {
            "regular"
        };
        let source_dll_name = if dll_name.contains("64") {
            "steam_api64.dll"
        } else {
            "steam_api.dll"
        };
        let source_dll = emu_path.join(variant).join(&arch).join(source_dll_name);

        if !source_dll.exists() {
            let err = format!("Emulator DLL not found: {:?}", source_dll);
            emit_log(app, &format!("  Error: {}", err));
            errors.push(err);
            continue;
        }

        // Copy emulator DLL
        match fs::copy(&source_dll, dll_path) {
            Ok(_) => {
                emit_log(
                    app,
                    &format!("   Replaced with Goldberg emulator ({})", variant),
                );
                steam_api_files.push(dll_path.to_string_lossy().to_string());
            }
            Err(e) => {
                let err = format!("Failed to copy emulator: {}", e);
                emit_log(app, &format!("  Error: {}", err));
                errors.push(err);
                continue;
            }
        }

        // Copy steamclient DLL if using experimental
        if config.use_experimental {
            let steamclient_name = if arch == "x64" {
                "steamclient64.dll"
            } else {
                "steamclient.dll"
            };
            let steamclient_src = emu_path
                .join("experimental")
                .join(&arch)
                .join(steamclient_name);
            if steamclient_src.exists() {
                let steamclient_dst = dll_folder.join(steamclient_name);
                if let Err(e) = fs::copy(&steamclient_src, &steamclient_dst) {
                    emit_log(
                        app,
                        &format!("  Warning: Could not copy {}: {}", steamclient_name, e),
                    );
                } else {
                    emit_log(app, &format!("  Copied {}", steamclient_name));
                }
            }
        }

        // Generate interfaces if requested
        if config.generate_interfaces {
            // Use the backup (original) file to generate interfaces
            let backup_path = dll_path.with_extension("dll.original");
            if backup_path.exists() {
                if let Err(e) = generate_interfaces_file(app, &backup_path, dll_folder, &emu_path) {
                    emit_log(app, &format!("  Warning: {}", e));
                }
            }
        }
    }

    // Create steam_settings for the first DLL's folder (usually the main game folder)
    let config_path = if let Some(first_dll) = dlls.first() {
        if let Some(folder) = first_dll.parent() {
            match create_steam_settings(folder, app_id, config) {
                Ok(path) => {
                    emit_log(app, &format!("\n Created steam_settings at: {:?}", path));
                    Some(path.to_string_lossy().to_string())
                }
                Err(e) => {
                    emit_log(app, &format!("\nError creating steam_settings: {}", e));
                    errors.push(e);
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let success = !steam_api_files.is_empty() && errors.is_empty();
    let message = if success {
        format!(
            "Successfully applied Goldberg Emulator to {} file(s)",
            steam_api_files.len()
        )
    } else if !steam_api_files.is_empty() {
        format!(
            "Applied with warnings: {}. Errors: {}",
            steam_api_files.len(),
            errors.join(", ")
        )
    } else {
        format!("Failed: {}", errors.join(", "))
    };

    emit_log(app, &format!("\n{}", message));

    GoldbergApplyResult {
        success,
        message,
        steam_api_files,
        config_path,
        backups_created,
    }
}

/// Generate crack-only files (DLLs without replacing originals)
pub fn generate_crack_only(
    app: &tauri::AppHandle,
    game_folder: &Path,
    output_folder: &Path,
    app_id: &str,
    config: &GoldbergConfig,
) -> Result<String, String> {
    emit_log(app, "Generating crack-only files...");

    let emu_path = find_emulator_path()?;

    // Create output folder
    fs::create_dir_all(output_folder).map_err(|e| e.to_string())?;

    // Scan original DLLs
    let dlls = scan_steam_api_dlls(game_folder)?;

    if dlls.is_empty() {
        return Err("No steam_api DLLs found in game folder".to_string());
    }

    let variant = if config.use_experimental {
        "experimental"
    } else {
        "regular"
    };

    for dll_path in &dlls {
        let dll_name = dll_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        let arch = detect_dll_architecture(dll_path)?;

        let source_dll_name = if dll_name.contains("64") {
            "steam_api64.dll"
        } else {
            "steam_api.dll"
        };
        let source_dll = emu_path.join(variant).join(&arch).join(source_dll_name);

        let dest_dll = output_folder.join(source_dll_name);
        fs::copy(&source_dll, &dest_dll).map_err(|e| e.to_string())?;
        emit_log(app, &format!("  Copied {} ({})", source_dll_name, arch));

        // Copy steamclient if experimental
        if config.use_experimental {
            let steamclient_name = if arch == "x64" {
                "steamclient64.dll"
            } else {
                "steamclient.dll"
            };
            let steamclient_src = emu_path
                .join("experimental")
                .join(&arch)
                .join(steamclient_name);
            if steamclient_src.exists() {
                let steamclient_dst = output_folder.join(steamclient_name);
                fs::copy(&steamclient_src, &steamclient_dst).ok();
                emit_log(app, &format!("  Copied {}", steamclient_name));
            }
        }
    }

    // Create steam_settings
    create_steam_settings(output_folder, app_id, config)?;
    emit_log(app, "  Created steam_settings folder");

    emit_log(
        app,
        &format!("\n Crack files generated at: {:?}", output_folder),
    );
    Ok(format!("Crack files generated at: {:?}", output_folder))
}

/// Restore original files from backups
pub fn restore_original_files(
    app: &tauri::AppHandle,
    game_folder: &Path,
) -> Result<String, String> {
    emit_log(app, "Restoring original files...");

    let mut restored = 0;

    fn restore_recursive(dir: &Path, restored: &mut i32) -> Result<(), String> {
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_dir() {
                restore_recursive(&path, restored)?;
            } else if path.is_file() {
                let name = path.file_name().unwrap().to_string_lossy();
                if name.ends_with(".dll.original") {
                    let original_path = path.with_extension("");
                    // original_path now ends with .dll
                    let final_path = original_path.with_extension("dll");
                    if let Err(_) = fs::copy(&path, &final_path) {
                        // Try the other way (path is already foo.dll.original, restore to foo.dll)
                        let restore_name = name.replace(".original", "");
                        let restore_path = path.parent().unwrap().join(restore_name);
                        fs::copy(&path, &restore_path).map_err(|e| e.to_string())?;
                    }
                    *restored += 1;
                }
            }
        }
        Ok(())
    }

    restore_recursive(game_folder, &mut restored)?;

    let msg = format!("Restored {} original file(s)", restored);
    emit_log(app, &msg);
    Ok(msg)
}

// --- Tauri Commands ---

#[tauri::command]
pub fn get_goldberg_languages() -> Vec<String> {
    LANGUAGES.iter().map(|s| s.to_string()).collect()
}

#[tauri::command]
pub fn scan_game_for_steam_api(game_folder: String) -> Result<Vec<String>, String> {
    let path = Path::new(&game_folder);
    let dlls = scan_steam_api_dlls(path)?;
    Ok(dlls
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub fn check_goldberg_available() -> Result<String, String> {
    find_emulator_path().map(|p| p.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn apply_goldberg_emulator(
    app: tauri::AppHandle,
    game_folder: String,
    app_id: String,
    config: GoldbergConfig,
) -> Result<GoldbergApplyResult, String> {
    let game_path = PathBuf::from(&game_folder);

    tokio::task::spawn_blocking(move || apply_emulator(&app, &game_path, &app_id, &config))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_goldberg_crack_only(
    app: tauri::AppHandle,
    game_folder: String,
    output_folder: String,
    app_id: String,
    config: GoldbergConfig,
) -> Result<String, String> {
    let game_path = PathBuf::from(&game_folder);
    let output_path = PathBuf::from(&output_folder);

    tokio::task::spawn_blocking(move || {
        generate_crack_only(&app, &game_path, &output_path, &app_id, &config)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn restore_goldberg_originals(
    app: tauri::AppHandle,
    game_folder: String,
) -> Result<String, String> {
    let game_path = PathBuf::from(&game_folder);

    tokio::task::spawn_blocking(move || restore_original_files(&app, &game_path))
        .await
        .map_err(|e| e.to_string())?
}
