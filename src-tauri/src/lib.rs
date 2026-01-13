// Main library entry point
mod appid;
mod commands;
mod download;
mod goldberg;
mod manifest;
mod steam;
mod steamless;

pub use commands::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(commands::AppState(std::sync::Mutex::new(
            commands::StagingState::default(),
        )))
        .invoke_handler(tauri::generate_handler![
            greet,
            check_dotnet,
            download_process,
            manual_download_process,
            get_game_info,
            search_games,
            update_appid_db,
            check_appid_db,
            // Depot selection commands
            prepare_morrenus_download,
            prepare_manual_download,
            execute_selected_download,
            cancel_staging,
            // Steamless commands
            steamless::scan_game_folder,
            steamless::run_steamless_cli,
            steamless::check_steamless_available,
            // Goldberg emulator commands
            goldberg::get_goldberg_languages,
            goldberg::scan_game_for_steam_api,
            goldberg::check_goldberg_available,
            goldberg::apply_goldberg_emulator,
            goldberg::generate_goldberg_crack_only,
            goldberg::restore_goldberg_originals,
            open_explorer_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
