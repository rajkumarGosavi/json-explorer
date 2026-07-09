mod commands;
mod dto;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::open_file,
            commands::get_root,
            commands::get_children,
            commands::get_node_value,
            commands::get_path,
            commands::search_start,
            commands::search_cancel,
            commands::close_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
