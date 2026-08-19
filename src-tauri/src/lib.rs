mod archive;
mod commands;
mod downloader;
mod events;
mod feedback;
mod hotkeys;
mod import;
mod logging;
mod media;
mod privacy;
mod recording;
mod rewind;
mod state;
mod sync;
mod tray;
mod windows;

use state::AppState;
use std::sync::OnceLock;
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_window_state::StateFlags;
use time::UtcOffset;

const ARG_MINIMIZED: &str = "--minimized";

const ARG_RECORD: &str = "record";

const ARG_IMPORT: &str = "import";

const ARG_PLAYER: &str = "player";

static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();

pub fn local_offset() -> UtcOffset {
    *LOCAL_OFFSET.get().unwrap_or(&UtcOffset::UTC)
}

#[cfg(target_os = "linux")]
fn keep_the_window_visible() {
    use std::os::unix::process::CommandExt;

    const KEY: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";

    if std::env::var_os(KEY).is_some() {
        return;
    }

    let Ok(exe) = std::env::current_exe() else {
        return;
    };

    let failure = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(KEY, "1")
        .exec();

    eprintln!("ZScribe could not restart with the software renderer: {failure}");
}

#[cfg(not(target_os = "linux"))]
fn keep_the_window_visible() {}

pub fn run() {
    keep_the_window_visible();

    let _ = LOCAL_OFFSET.set(UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC));

    let args: Vec<String> = std::env::args().collect();
    let start_hidden = args.iter().any(|arg| arg == ARG_MINIMIZED);

    logging::init();
    tracing::info!(
        version = zscribe_core::VERSION,
        gpu = zscribe_stt::gpu_support_compiled_in(),
        "starting ZScribe"
    );

    let state = match AppState::load() {
        Ok(state) => state,
        Err(err) => {
            tracing::error!(%err, "could not load application state");
            eprintln!("ZScribe could not start: {err}");
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .manage(state)
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if argv.iter().any(|arg| arg == ARG_RECORD) {
                recording::toggle(app.clone());
            } else if let Some(index) = argv.iter().position(|arg| arg == ARG_IMPORT) {
                match argv.get(index + 1) {
                    Some(target) => {
                        let app = app.clone();
                        let target = target.clone();

                        std::thread::spawn(move || {
                            let is_link =
                                target.starts_with("http://") || target.starts_with("https://");

                            let result = if is_link {
                                import::link(&app, target)
                            } else {
                                import::file(&app, std::path::PathBuf::from(target))
                            };

                            if let Err(err) = result {
                                tracing::error!(
                                    code = %err.code,
                                    message = %err.message,
                                    remedy = %err.remedy,
                                    "import failed"
                                );
                            }
                        });
                    }
                    None => tracing::error!("`zscribe import` needs a file or a link"),
                }
            } else if let Some(index) = argv.iter().position(|arg| arg == ARG_PLAYER) {
                let wanted = argv.get(index + 1).cloned().or_else(|| {
                    app.state::<AppState>()
                        .recordings
                        .lock()
                        .expect("recordings lock poisoned")
                        .list(1)
                        .ok()
                        .and_then(|recordings| recordings.first().map(|r| r.id.clone()))
                });

                match wanted {
                    Some(id) => {
                        if let Err(err) = commands::open_player(app.clone(), id, None) {
                            tracing::error!(message = %err.message, "could not open the player");
                        }
                    }
                    None => tracing::error!("there is no recording to open the player on"),
                }
            } else {
                windows::show_main(app);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .with_denylist(&[events::RECORDER_WINDOW])
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![ARG_MINIMIZED]),
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_templates,
            commands::derive_template,
            commands::get_capabilities,
            commands::scan_machine,
            commands::recommend_model,
            commands::get_paths,
            commands::app_version,
            commands::gpu_support,
            commands::validate_hotkey,
            commands::get_hotkey_status,
            commands::list_models,
            commands::set_api_key,
            commands::has_api_key,
            commands::get_secret_backend,
            commands::list_catalogue,
            commands::suggest_llm,
            commands::llm_acceleration,
            commands::install_llm,
            commands::cancel_llm_install,
            commands::list_whisper_models,
            commands::installed_models,
            commands::download_model,
            commands::cancel_model_download,
            commands::remove_model,
            commands::list_input_devices,
            commands::list_system_audio,
            commands::check_sources,
            commands::start_recording,
            commands::stop_recording,
            commands::toggle_recording,
            commands::pause_recording,
            commands::resume_recording,
            commands::recording_status,
            commands::cancel_processing,
            commands::list_recordings,
            commands::search_recordings,
            commands::archive_status,
            commands::index_archive,
            commands::ask_archive,
            commands::get_recording,
            commands::rename_recording,
            commands::delete_recording,
            commands::delete_all_recordings,
            commands::recording_markdown,
            commands::ask,
            commands::resummarise,
            commands::retranscribe,
            commands::edit_transcript_line,
            commands::rename_speaker,
            commands::recording_subtitles,
            commands::export_recording,
            commands::set_tags,
            commands::list_tags,
            commands::import_file,
            commands::import_link,
            commands::link_support,
            commands::importable_extensions,
            commands::open_player,
            commands::player_recording,
            commands::audio_url,
            commands::audio_peaks,
            commands::report_error,
            commands::show_main_window,
            commands::set_autostart,
            commands::is_autostart_enabled,
        ])
        .setup(move |app| {
            tray::install(app)?;

            let handle = app.handle().clone();

            match media::start(&handle) {
                Ok(media) => {
                    let _ = handle.state::<AppState>().media.set(media);
                }
                Err(err) => tracing::error!(%err, "playback will not be available"),
            }

            sync::watch(handle.clone());

            rewind::reconcile(&handle);

            let accelerator = handle.state::<AppState>().settings().hotkey;
            if let Err(err) = hotkeys::register_and_record(&handle, &accelerator) {
                tracing::warn!(%err, %accelerator, "could not register the global hotkey");
            }

            if start_hidden {
                windows::hide_main(&handle);
            } else {
                windows::show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == events::PLAYER_WINDOW {
                    api.prevent_close();
                    let _ = window.hide();
                    let _ = window.app_handle().emit(events::PLAYER_HIDDEN, ());
                    return;
                }

                if window.label() != events::MAIN_WINDOW {
                    return;
                }

                let app = window.app_handle();

                if app.state::<AppState>().settings().system.minimize_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start ZScribe")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                hotkeys::unregister_all(app);
            }
        });
}
