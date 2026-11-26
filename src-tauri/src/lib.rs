pub mod backup;
pub mod browser;
pub mod commands;
pub mod db;
pub mod downloads;
pub mod manga;
pub mod plugins;

use std::sync::Arc;

use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        // Remember size/position/maximized, but not the reader's fullscreen mode.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::all() & !tauri_plugin_window_state::StateFlags::FULLSCREEN,
                )
                .build(),
        )
        .setup(|app| {
            browser::init(app.handle().clone());
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db = db::Db::open(&data_dir.join("kizuna.db"))?;
            let plugins = plugins::PluginStore::new(data_dir.join("sources"))?;
            let downloads = downloads::Downloads::new(data_dir.join("downloads"))?;
            // Downloads from before readable names: <novel id>/<chapter id> -> <title>/<chapter>.
            downloads.migrate_numeric(&db);
            let engine_dir = data_dir.join("manga-engine");
            let manga = manga::ExtHost::new(manga::EnginePaths::resolve(&engine_dir, data_dir.join("manga")));
            app.manage(Arc::new(AppState {
                db,
                plugins,
                downloads,
                manga,
                manga_tmp: data_dir.join("manga-tmp"),
                image_cache: data_dir.join("image-cache"),
            }));
            downloads::Downloads::start(app.handle().clone());
            // The UI shows the (initially hidden) window once its splash is painted. Safety
            // net in case the frontend fails to load: show it anyway after a few seconds.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(4));
                if let Some(w) = handle.get_webview_window("main") {
                    let _ = w.show();
                }
            });
            Ok(())
        })
        // Manga covers/pages, fetched by the extension host with each source's headers.
        .register_asynchronous_uri_scheme_protocol("kimg", |ctx, request, responder| {
            let state = ctx.app_handle().state::<Arc<AppState>>().inner().clone();
            let uri = request.uri().to_string();
            std::thread::spawn(move || {
                responder.respond(manga::images::handle(&state.manga, &state.image_cache, &state.downloads, &state.db, &uri));
            });
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
            commands::backup_import,
            commands::manga_engine_status,
            commands::manga_engine_install,
            commands::manga_engine_remove,
            commands::manga_repo_list,
            commands::manga_extensions,
            commands::manga_extension_install,
            commands::manga_extension_uninstall,
            commands::manga_browse,
            commands::manga_pages,
            commands::manga_sources_cached,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            // Closing the main window quits, even while the (hidden) browser window exists.
            tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::Destroyed, .. } if label == "main" => {
                app.exit(0);
            }
            // Stop the extension host with the app.
            tauri::RunEvent::Exit => app.state::<Arc<AppState>>().manga.stop(),
            _ => {}
        });
}
