mod commands;
mod merge_manifest;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::scan_package_resources,
            commands::get_resource_preview,
            commands::analyze_remove_selection,
            commands::remove_selected_from_merged,
            commands::collect_merge_package_files,
            commands::merge_packages_with_manifest,
            commands::restore_manifest_packages,
            commands::export_selected_resources,
            commands::zip_selected_resources
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
