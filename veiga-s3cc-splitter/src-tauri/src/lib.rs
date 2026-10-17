mod commands;
mod system_appearance;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            system_appearance::get_system_appearance,
            commands::scan_package_resources,
            commands::get_resource_preview,
            commands::analyze_remove_selection,
            commands::remove_selected_from_merged,
            commands::export_selected_resources,
            commands::zip_selected_resources
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
