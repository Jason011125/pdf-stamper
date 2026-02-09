mod commands;
mod pdf;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            commands::open_pdfs,
            commands::render_page,
            commands::read_file_bytes,
            commands::stamp_pdfs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
