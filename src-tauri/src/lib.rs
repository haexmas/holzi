pub mod adapters;
pub mod catalog;
pub mod chat;
pub mod error;
pub mod hardware;
pub mod identity;
pub mod instances;
pub mod llm;
pub mod models;
pub mod providers;
pub mod state;
pub mod state_utils;
pub mod storage;

pub use error::{HolziError, Result};
pub use state::{ActiveInstanceHandle, AppState};

use catalog::list_catalog;
use chat::commands::{
    abort_current_generation, active_model_info, load_model, send_message, unload_local_model,
};
use chat::session::ChatState;
use chat::thread_commands::{create_thread, list_messages, list_threads};
use hardware::get_hardware_info;
use instances::{
    cleanup_orphans_on_startup, close_instance, create_instance, list_instances, open_instance,
};
use models::commands::{
    delete_installed_model, download_model_from_catalog, download_model_from_hf,
    import_model_from_file, list_installed_models,
};
use providers::{
    add_provider, delete_provider, list_provider_models, list_providers, refresh_provider_models,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Builds and starts the holzi Tauri application.
pub fn run() {
    let builder = tauri::Builder::default().manage(AppState::new());
    let builder = builder.manage(ChatState::new());
    builder
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // Orphan cleanup before any command handler can run. A
            // failure here is logged but does not abort startup — the
            // orphan just stays around, and `list_instances` filters it
            // out because of its sibling `.pending` marker.
            if let Err(e) = cleanup_orphans_on_startup(app.handle()) {
                log::warn!("startup orphan cleanup failed: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_instances,
            create_instance,
            open_instance,
            close_instance,
            get_hardware_info,
            list_catalog,
            add_provider,
            list_providers,
            delete_provider,
            refresh_provider_models,
            list_provider_models,
            download_model_from_catalog,
            download_model_from_hf,
            import_model_from_file,
            list_installed_models,
            delete_installed_model,
            load_model,
            unload_local_model,
            active_model_info,
            send_message,
            abort_current_generation,
            create_thread,
            list_threads,
            list_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
