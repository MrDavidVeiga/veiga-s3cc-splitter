mod commands;
mod merge_manifest;
mod system_appearance;

use tauri::Manager;

#[cfg(target_os = "windows")]
fn set_windows_app_identity() {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let app_id: Vec<u16> = "com.veiga.s3ccsplitter\0".encode_utf16().collect();
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(PCWSTR(app_id.as_ptr()));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    set_windows_app_identity();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let icon = tauri::image::Image::from_bytes(include_bytes!(
                "../../../sims3-cli/assets/icons/128x128.png"
            ))?;
            if let Some(window) = app.get_webview_window("main") {
                window.set_icon(icon)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            system_appearance::get_system_appearance,
            commands::scan_package_resources,
            commands::cancel_package_operation,
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
