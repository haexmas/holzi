pub mod adapters;
#[cfg(feature = "voice")]
pub mod audio;
pub mod catalog;
pub mod chat;
pub mod device;
pub mod error;
pub mod hardware;
pub mod identity;
pub mod instances;
pub mod llm;
pub mod model_capabilities;
pub mod models;
pub mod providers;
pub mod state;
pub mod state_utils;
pub mod storage;
// Unconditional (unlike `audio`, above): `stt::mod`/`stt::interrupt` are
// plain, dependency-free types the `llm-cpu`-only `stt::local` adapter
// (candle Whisper) and future `voice`-gated commands both need. Keeping
// them out of the `voice` gate means `stt::local`'s tests run under
// `--features llm-cpu` alone, without pulling in `cpal` (and its ALSA/
// CoreAudio/WASAPI system dependency) at all.
pub mod stt;
pub mod vault_gate;
// Unconditional like `stt`, above — see `voice.rs`'s module doc for why
// the stub commands live in the same file as the real ones.
pub mod voice;

pub use error::{HolziError, Result};
pub use state::{ActiveInstanceHandle, AppState};

use catalog::commands::catalog_recommend_tiers;
use catalog::list_catalog;
use chat::commands::{
    abort_current_generation, inspect_attachment, respond_tool_permission, send_message,
};
use chat::default_model::{model_load_status, resolve_default_model};
use chat::model_loading::{
    active_model_info, load_model, load_model_with_integrity_override, unload_local_model,
};
use chat::session::ChatState;
use chat::thread_commands::{
    create_thread, delete_thread, list_messages, list_threads, rename_thread,
};
use device::commands::{current_device_info, update_device_alias};
use hardware::get_hardware_info;
use instances::{
    cleanup_orphans_on_startup, close_instance, create_instance, list_instances, open_instance,
};
use models::commands::{
    check_huggingface_model_updates, delete_installed_model, download_model_from_catalog,
    download_model_from_hf, get_huggingface_model_details, import_model_from_file,
    install_huggingface_update, list_installed_models, preview_huggingface_install,
    search_huggingface_models,
};
use providers::connect::{connect_cli_delegate, submit_cli_delegate_code, DelegateConnectState};
use providers::{
    add_provider, delete_provider, list_provider_models, list_providers, refresh_provider_models,
};
use storage::preferences_commands::{clear_pref, get_pref, set_pref};
use stt::commands::{
    download_stt_model, list_installed_stt_models, list_stt_catalog, stt_recommend_tiers,
};
use voice::{
    cancel_voice_recording, invalidate_stt_model_cache, start_voice_recording, stop_voice_recording,
};

/// Undoes, for this process's children only, the Nix devShell's
/// `LD_LIBRARY_PATH` export (flake.nix's shellHook) — present only when
/// `build.rs`'s `configure_nix_devshell_linker` also overrode this binary's
/// own dynamic-linker interpreter to the host's, which is when that variable
/// is actually needed (to resolve the Nix-provided GTK stack the interpreter
/// override doesn't cover) and, unmodified, would otherwise leak into every
/// child process GTK/WebKit forks (WebKitWebProcess, WebKitNetworkProcess,
/// Mesa's GBM driver loader) — all built against the *host's* glibc, not
/// Nix's, so they'd hit the same `GLIBC_PRIVATE` symbol clash the interpreter
/// override exists to avoid on this binary. Safe to clear this late: the
/// dynamic linker already consulted it to resolve this process's own NEEDED
/// libraries before `main` ever ran; removing it from the environment now
/// only stops it from propagating to processes spawned from here on.
fn clear_inherited_nix_library_path() {
    if option_env!("HOLZI_NIX_DEVSHELL_LINKER").is_some() {
        std::env::remove_var("LD_LIBRARY_PATH");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
/// Builds and starts the holzi Tauri application.
pub fn run() {
    clear_inherited_nix_library_path();
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("--internal-cli-delegate-approval-bridge") {
        let Some(socket_flag) = args.next().filter(|arg| arg == "--socket") else {
            eprintln!("missing --socket for CLI delegate approval bridge");
            return;
        };
        let _ = socket_flag;
        let Some(socket_path) = args.next() else {
            eprintln!("missing socket path for CLI delegate approval bridge");
            return;
        };
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                eprintln!("failed to start approval bridge runtime: {error}");
                return;
            }
        };
        if let Err(error) = runtime.block_on(
            adapters::cli_delegate::permission_mcp_server::run_bridge_process(
                std::path::Path::new(&socket_path),
            ),
        ) {
            eprintln!("CLI delegate approval bridge failed: {error}");
        }
        return;
    }
    let builder = tauri::Builder::default().manage(AppState::new());
    let builder = builder.manage(ChatState::new());
    let builder = builder.manage(DelegateConnectState::new());
    let builder = builder.manage(voice::VoiceState::new());
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            catalog_recommend_tiers,
            add_provider,
            list_providers,
            delete_provider,
            connect_cli_delegate,
            submit_cli_delegate_code,
            refresh_provider_models,
            list_provider_models,
            download_model_from_catalog,
            download_model_from_hf,
            import_model_from_file,
            list_installed_models,
            delete_installed_model,
            search_huggingface_models,
            get_huggingface_model_details,
            preview_huggingface_install,
            install_huggingface_update,
            check_huggingface_model_updates,
            load_model,
            load_model_with_integrity_override,
            unload_local_model,
            active_model_info,
            model_load_status,
            resolve_default_model,
            send_message,
            inspect_attachment,
            abort_current_generation,
            respond_tool_permission,
            create_thread,
            list_threads,
            list_messages,
            rename_thread,
            delete_thread,
            current_device_info,
            update_device_alias,
            get_pref,
            set_pref,
            clear_pref,
            start_voice_recording,
            stop_voice_recording,
            cancel_voice_recording,
            invalidate_stt_model_cache,
            list_stt_catalog,
            stt_recommend_tiers,
            list_installed_stt_models,
            download_stt_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
