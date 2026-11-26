pub mod commands;
pub mod db;
pub mod downloads;
pub mod plugins;

use std::sync::Arc;

use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Remember size/position/maximized, but not the reader's fullscreen mode.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all() & !tauri_plugin_window_state::StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = db::Db::open(&data_dir.join("kizuna.db"))?;
            let plugins = plugins::PluginStore::new(data_dir.join("sources"))?;
            let downloads = downloads::Downloads::new(data_dir.join("downloads"))?;
            app.manage(Arc::new(AppState { db, plugins, downloads }));
            downloads::Downloads::start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::sources_installed,
            commands::sources_index,
            commands::source_install,
            commands::source_uninstall,
            commands::source_browse,
            commands::novel_get,
            commands::novel_refresh,
            commands::novel_set_favorite,
            commands::novel_set_categories,
            commands::chapter_get,
            commands::chapter_content,
            commands::chapter_save_progress,
            commands::chapters_set_read,
            commands::chapters_set_bookmark,
            commands::library_list,
            commands::library_update,
            commands::categories_list,
            commands::category_create,
            commands::category_rename,
            commands::category_delete,
            commands::categories_reorder,
            commands::history_list,
            commands::history_remove,
            commands::history_clear,
            commands::updates_list,
            commands::downloads_enqueue,
            commands::downloads_cancel,
            commands::downloads_clear,
            commands::downloads_set_paused,
            commands::downloads_state,
            commands::downloads_delete,
            commands::downloads_delete_novel,
            commands::downloads_size,
            commands::settings_get,
            commands::settings_set,
            commands::now_ms,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
