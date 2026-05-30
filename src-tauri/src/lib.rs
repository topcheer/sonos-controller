use std::sync::Mutex;

pub mod commands;
pub mod ssdp;
pub mod upnp;
pub mod music_services;

#[derive(Default)]
pub struct AppState {
    pub selected_device: Option<String>,
}

pub use ssdp::SonosDevice;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .manage(Mutex::new(AppState::default()))
        .invoke_handler(tauri::generate_handler![
            commands::discover_devices,
            commands::get_zone_groups,
            commands::get_queue,
            commands::browse_directory,
            commands::play_from_queue,
            commands::play_uri,
            commands::get_transport_info,
            commands::get_position_info,
            commands::get_media_info,
            commands::play,
            commands::pause,
            commands::stop,
            commands::next_track,
            commands::previous_track,
            commands::seek,
            commands::get_play_mode,
            commands::set_play_mode,
            commands::get_crossfade,
            commands::set_crossfade,
            commands::get_transport_settings,
            commands::get_sleep_timer,
            commands::set_sleep_timer,
            commands::get_volume,
            commands::set_volume,
            commands::get_mute,
            commands::set_mute,
            commands::get_eq,
            commands::set_eq,
            commands::get_bass,
            commands::set_bass,
            commands::get_treble,
            commands::set_treble,
            commands::get_loudness,
            commands::set_loudness,
            commands::join_group,
            commands::leave_group,
            commands::open_url,
            commands::search_qq_music,
            // Music Services (SMAPI)
            commands::list_music_services,
            commands::get_household_id,
            commands::get_music_service_app_link,
            commands::get_music_service_auth_token,
            commands::browse_music_service,
            commands::get_music_service_root,
            commands::search_music_service,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
