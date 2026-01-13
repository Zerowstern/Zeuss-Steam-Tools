// Steamless CLI integration for removing Steam DRM
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::Emitter;

/// Steamless options for CLI execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SteamlessOptions {
    pub quiet: bool,
    pub keep_bind: bool,
    pub keep_stub: bool,
    pub dump_payload: bool,
    pub dump_drmp: bool,
    pub realign: bool,
    pub recalc_checksum: bool,
    pub experimental: bool,
}

/// Result of processing a single executable
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamlessResult {
    pub file: String,
    pub success: bool,
    pub message: String,
    pub output_file: Option<String>,
}

/// Result of batch processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchSteamlessResult {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<SteamlessResult>,
}

fn emit_log(app: &tauri::AppHandle, msg: &str) {
    let _ = app.emit("steamless-log", msg);
}

/// Find Steamless CLI executable
fn find_steamless_cli() -> Result<PathBuf, String> {
    // Check common locations
    let candidates = vec![
        "Steamless.CLI.exe",
        "Steamless.v3.1.0.5.-.by.atom0s/Steamless.CLI.exe",
        "../Steamless.v3.1.0.5.-.by.atom0s/Steamless.CLI.exe",
        "tools/Steamless.CLI.exe",
    ];

    for c in &candidates {
        let p = Path::new(c);
        if p.exists() {
            return Ok(p.to_path_buf());
        }
    }

    // Also check relative to exe location
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

    let cwd = std::env::current_dir().unwrap_or_default();
    Err(format!(
        "Steamless.CLI.exe not found. Searched from: {:?}",
        cwd
    ))
}

/// Common non-game executables to filter out
const EXCLUDED_EXE_PATTERNS: &[&str] = &[
    "crashhandler",
    "unitycrashhandler",
    "crashreporter",
    "crashpad",
    "unins", // uninstallers
    "setup",
    "installer",
    "uninstall",
    "redist",
    "vcredist",
    "dxsetup",
    "dotnet",
    "ue4prereq", // Unreal prerequisites
    "eaborig",   // EA Origin
    "eadesktop",
    "easyanticheat",
    "beclient", // BattlEye
    "beservice",
];

/// Check if an executable should be excluded
fn should_exclude_exe(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    EXCLUDED_EXE_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

/// Scan a directory for executable files
pub fn scan_executables(dir_path: &Path) -> Result<Vec<PathBuf>, String> {
    let mut executables = Vec::new();

    if !dir_path.exists() {
        return Err(format!("Directory does not exist: {:?}", dir_path));
    }

    if !dir_path.is_dir() {
        return Err(format!("Path is not a directory: {:?}", dir_path));
    }

    fn scan_recursive(dir: &Path, exes: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = fs::read_dir(dir).map_err(|e| e.to_string())?;

        for entry in entries {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively scan subdirectories
                scan_recursive(&path, exes)?;
            } else if path.is_file() {
                // Check if it's an executable
                if let Some(ext) = path.extension() {
                    if ext.to_ascii_lowercase() == "exe" {
                        // Filter out common non-game executables
                        if let Some(name) = path.file_name() {
                            let name_str = name.to_string_lossy();
                            if !should_exclude_exe(&name_str) {
                                exes.push(path);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    scan_recursive(dir_path, &mut executables)?;
    Ok(executables)
}

/// Build CLI arguments from options
fn build_cli_args(file_path: &Path, options: &SteamlessOptions) -> Vec<String> {
    let mut args = Vec::new();

    // Add the file path first
    args.push(file_path.to_string_lossy().to_string());

    // Add options
    if options.quiet {
        args.push("--quiet".to_string());
    }
    if options.keep_bind {
        args.push("--keepbind".to_string());
    }
    if options.keep_stub {
        args.push("--keepstub".to_string());
    }
    if options.dump_payload {
        args.push("--dumppayload".to_string());
    }
    if options.dump_drmp {
        args.push("--dumpdrmp".to_string());
    }
    if options.realign {
        args.push("--realign".to_string());
    }
    if options.recalc_checksum {
        args.push("--recalcchecksum".to_string());
    }
    if options.experimental {
        args.push("--exp".to_string());
    }

    args
}

/// Process a single executable with Steamless
pub fn process_single_exe(
    app: &tauri::AppHandle,
    exe_path: &Path,
    options: &SteamlessOptions,
) -> SteamlessResult {
    let file_name = exe_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    emit_log(app, &format!("Processing: {}", file_name));

    // Find Steamless CLI
    let steamless_path = match find_steamless_cli() {
        Ok(p) => p,
        Err(e) => {
            return SteamlessResult {
                file: file_name,
                success: false,
                message: e,
                output_file: None,
            };
        }
    };

    // Build arguments
    let args = build_cli_args(exe_path, options);

    // Run Steamless CLI
    let result = Command::new(&steamless_path)
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match result {
        Ok(c) => c,
        Err(e) => {
            return SteamlessResult {
                file: file_name,
                success: false,
                message: format!("Failed to start Steamless: {}", e),
                output_file: None,
            };
        }
    };

    // Capture output
    let mut output_lines = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                emit_log(app, &format!("  {}", l));
                output_lines.push(l);
            }
        }
    }

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            return SteamlessResult {
                file: file_name,
                success: false,
                message: format!("Failed to wait for Steamless: {}", e),
                output_file: None,
            };
        }
    };

    // Check for output file (Steamless creates filename.unpacked.exe)
    let output_file_path = exe_path.with_extension("unpacked.exe");
    let output_exists = output_file_path.exists();

    // Also check for the common pattern: filename.exe.unpacked.exe
    let alt_output_path = PathBuf::from(format!("{}.unpacked.exe", exe_path.display()));
    let alt_exists = alt_output_path.exists();

    let actual_output = if output_exists {
        Some(output_file_path.to_string_lossy().to_string())
    } else if alt_exists {
        Some(alt_output_path.to_string_lossy().to_string())
    } else {
        None
    };

    // Check if the output contains "All unpackers failed" - means file is not SteamStub protected
    let not_protected = output_lines.iter().any(|line| {
        line.contains("All unpackers failed")
            || line.contains("not protected")
            || line.contains("No valid")
    });

    let success = actual_output.is_some();

    let mut message = if success {
        " Successfully unpacked - SteamStub DRM removed".to_string()
    } else if not_protected {
        " Not SteamStub protected - no action needed (may only need emulator)".to_string()
    } else if status.success() {
        " Completed but no output file (may not be Steam protected)".to_string()
    } else {
        " Processing failed - check if this is a valid game executable".to_string()
    };

    // If successful, swap files
    if success {
        if let Some(ref unpacked_path_str) = actual_output {
            let unpacked_path = PathBuf::from(unpacked_path_str);
            let backup_path = exe_path.with_extension("exe.bak");

            // 1. Rename original to .bak
            let rename_original = if backup_path.exists() {
                // If backup already exists, we assume current exe is already processed or we shouldn't overwrite backup
                // Just delete current exe to make way for new one? Or rename to .bak2?
                // For safety, let's just delete the current exe since we have the unpacked one ready
                match fs::remove_file(exe_path) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("Failed to remove original: {}", e)),
                }
            } else {
                match fs::rename(exe_path, &backup_path) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(format!("Failed to backup original: {}", e)),
                }
            };

            if let Err(e) = rename_original {
                message = format!(" Unpacked but failed to backup original: {}", e);
            } else {
                // 2. Rename unpacked to original name
                match fs::rename(&unpacked_path, exe_path) {
                    Ok(_) => {
                        message =
                            " SteamStub removed & replaced original (backup created)".to_string();
                    }
                    Err(e) => {
                        // Try to restore backup
                        if backup_path.exists() {
                            let _ = fs::rename(&backup_path, exe_path);
                        }
                        message = format!(" Unpacked but failed to replace original: {}", e);
                    }
                }
            }
        }
    }

    emit_log(app, &format!("  {}", message));

    SteamlessResult {
        file: file_name,
        success,
        message,
        output_file: actual_output,
    }
}

/// Process multiple executables in batch
pub fn process_batch(
    app: &tauri::AppHandle,
    exe_paths: &[PathBuf],
    options: &SteamlessOptions,
) -> BatchSteamlessResult {
    let total = exe_paths.len();
    let mut successful = 0;
    let mut failed = 0;
    let mut results = Vec::new();

    emit_log(
        app,
        &format!("Starting batch processing of {} executables...", total),
    );

    for (i, exe_path) in exe_paths.iter().enumerate() {
        emit_log(app, &format!("\n[{}/{}] Processing...", i + 1, total));
        let result = process_single_exe(app, exe_path, options);

        if result.success {
            successful += 1;
        } else {
            failed += 1;
        }

        results.push(result);
    }

    emit_log(
        app,
        &format!(
            "\nBatch complete: {} successful, {} failed out of {} total",
            successful, failed, total
        ),
    );

    BatchSteamlessResult {
        total,
        successful,
        failed,
        results,
    }
}

// --- Tauri Commands ---

#[tauri::command]
pub fn scan_game_folder(folder_path: String) -> Result<Vec<String>, String> {
    let path = Path::new(&folder_path);
    let executables = scan_executables(path)?;

    Ok(executables
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
pub async fn run_steamless_cli(
    app: tauri::AppHandle,
    exe_paths: Vec<String>,
    options: SteamlessOptions,
) -> Result<BatchSteamlessResult, String> {
    let paths: Vec<PathBuf> = exe_paths.into_iter().map(PathBuf::from).collect();

    tokio::task::spawn_blocking(move || process_batch(&app, &paths, &options))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn check_steamless_available() -> Result<String, String> {
    find_steamless_cli().map(|p| p.to_string_lossy().to_string())
}
