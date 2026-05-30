use crate::ssdp;
use crate::upnp;
use crate::music_services;

#[tauri::command]
pub async fn discover_devices(timeout: Option<u64>) -> Result<Vec<ssdp::SonosDevice>, String> {
    ssdp::discover_sonos(timeout.unwrap_or(5)).await
}

#[tauri::command]
pub async fn get_zone_groups(ip: String) -> Result<Vec<ssdp::ZoneGroup>, String> {
    ssdp::get_zone_groups(&ip).await
}

#[tauri::command]
pub async fn get_queue(ip: String, start: Option<u32>, limit: Option<u32>) -> Result<Vec<ssdp::QueueItem>, String> {
    ssdp::get_queue(&ip, start.unwrap_or(0), limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn browse_directory(ip: String, object_id: String, start: Option<u32>, limit: Option<u32>) -> Result<Vec<ssdp::BrowserItem>, String> {
    ssdp::browse_directory(&ip, &object_id, start.unwrap_or(0), limit.unwrap_or(200)).await
}

#[tauri::command]
pub async fn play_from_queue(ip: String, track_index: u32) -> Result<(), String> {
    ssdp::play_from_queue(&ip, track_index).await
}

#[tauri::command]
pub async fn play_uri(ip: String, uri: String, metadata: Option<String>) -> Result<(), String> {
    ssdp::play_uri(&ip, &uri, metadata.as_deref().unwrap_or("")).await
}

// Transport
#[tauri::command]
pub async fn get_transport_info(ip: String) -> Result<upnp::TransportInfo, String> {
    upnp::UpnpControl::new(&ip).get_transport_info().await
}

#[tauri::command]
pub async fn get_position_info(ip: String) -> Result<upnp::PositionInfo, String> {
    upnp::UpnpControl::new(&ip).get_position_info().await
}

#[tauri::command]
pub async fn get_media_info(ip: String) -> Result<upnp::MediaInfo, String> {
    upnp::UpnpControl::new(&ip).get_media_info().await
}

#[tauri::command]
pub async fn play(ip: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).play().await }

#[tauri::command]
pub async fn pause(ip: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).pause().await }

#[tauri::command]
pub async fn stop(ip: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).stop().await }

#[tauri::command]
pub async fn next_track(ip: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).next().await }

#[tauri::command]
pub async fn previous_track(ip: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).previous().await }

#[tauri::command]
pub async fn seek(ip: String, target: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).seek(&target).await }

// PlayMode + Crossfade
#[tauri::command]
pub async fn get_play_mode(ip: String) -> Result<upnp::PlayMode, String> { upnp::UpnpControl::new(&ip).get_play_mode().await }

#[tauri::command]
pub async fn set_play_mode(ip: String, mode: String) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_play_mode(&mode).await }

#[tauri::command]
pub async fn get_crossfade(ip: String) -> Result<upnp::CrossfadeMode, String> { upnp::UpnpControl::new(&ip).get_crossfade().await }

#[tauri::command]
pub async fn set_crossfade(ip: String, on: bool) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_crossfade(on).await }

#[tauri::command]
pub async fn get_transport_settings(ip: String) -> Result<upnp::TransportSettings, String> {
    upnp::UpnpControl::new(&ip).get_transport_settings().await
}

// Sleep timer
#[tauri::command]
pub async fn get_sleep_timer(ip: String) -> Result<upnp::SleepTimer, String> { upnp::UpnpControl::new(&ip).get_sleep_timer().await }

#[tauri::command]
pub async fn set_sleep_timer(ip: String, seconds: u32) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_sleep_timer(seconds).await }

// Volume
#[tauri::command]
pub async fn get_volume(ip: String) -> Result<u8, String> { upnp::UpnpControl::new(&ip).get_volume().await }

#[tauri::command]
pub async fn set_volume(ip: String, volume: u8) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_volume(volume).await }

#[tauri::command]
pub async fn get_mute(ip: String) -> Result<bool, String> { upnp::UpnpControl::new(&ip).get_mute().await }

#[tauri::command]
pub async fn set_mute(ip: String, mute: bool) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_mute(mute).await }

// EQ / Tone
#[tauri::command]
pub async fn get_eq(ip: String, eq_type: String) -> Result<upnp::EqData, String> { upnp::UpnpControl::new(&ip).get_eq(&eq_type).await }

#[tauri::command]
pub async fn set_eq(ip: String, eq_type: String, value: i16) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_eq(&eq_type, value).await }

#[tauri::command]
pub async fn get_bass(ip: String) -> Result<i16, String> { upnp::UpnpControl::new(&ip).get_bass().await }

#[tauri::command]
pub async fn set_bass(ip: String, value: i16) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_bass(value).await }

#[tauri::command]
pub async fn get_treble(ip: String) -> Result<i16, String> { upnp::UpnpControl::new(&ip).get_treble().await }

#[tauri::command]
pub async fn set_treble(ip: String, value: i16) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_treble(value).await }

#[tauri::command]
pub async fn get_loudness(ip: String) -> Result<bool, String> { upnp::UpnpControl::new(&ip).get_loudness().await }

#[tauri::command]
pub async fn set_loudness(ip: String, on: bool) -> Result<(), String> { upnp::UpnpControl::new(&ip).set_loudness(on).await }

// Group management
#[tauri::command]
pub async fn join_group(ip: String, coordinator_ip: String) -> Result<(), String> {
    upnp::UpnpControl::new(&ip).join_group(&coordinator_ip).await
}

#[tauri::command]
pub async fn leave_group(ip: String) -> Result<(), String> {
    upnp::UpnpControl::new(&ip).leave_group().await
}

// Open URL in system browser
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    open::that(&url).map_err(|e| format!("Failed to open URL: {e}"))
}

// Search QQ Music directly via web API
#[tauri::command]
pub async fn search_qq_music(term: String) -> Result<Vec<crate::music_services::SmapiItem>, String> {
    let client = reqwest::Client::new();
    crate::music_services::search_qq_music(&client, &term, 30).await
}

// ── Music Services (SMAPI) ──────────────────────────────────────

#[tauri::command]
pub async fn list_music_services(ip: String) -> Result<Vec<music_services::MusicService>, String> {
    let client = reqwest::Client::new();
    music_services::list_music_services(&client, &ip).await
}

#[tauri::command]
pub async fn get_household_id(ip: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    music_services::get_household_id(&client, &ip).await
}

#[tauri::command]
pub async fn get_music_service_app_link(service_uri: String, household_id: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    music_services::get_app_link(&client, &service_uri, &household_id).await
}

#[tauri::command]
pub async fn get_music_service_auth_token(service_uri: String, household_id: String, link_code: String, link_device_id: String) -> Result<String, String> {
    let client = reqwest::Client::new();
    music_services::get_device_auth_token(&client, &service_uri, &household_id, &link_code, &link_device_id).await
}

#[tauri::command]
pub async fn browse_music_service(service_uri: String, auth_token: String, parent_id: String, index: Option<u32>, count: Option<u32>) -> Result<music_services::SmapiBrowseResult, String> {
    let client = reqwest::Client::new();
    music_services::browse_music_service(&client, &service_uri, &auth_token, &parent_id, index.unwrap_or(0), count.unwrap_or(100)).await
}

#[tauri::command]
pub async fn get_music_service_root(service_uri: String, auth_token: String) -> Result<music_services::SmapiBrowseResult, String> {
    let client = reqwest::Client::new();
    music_services::get_service_root(&client, &service_uri, &auth_token).await
}

#[tauri::command]
pub async fn search_music_service(service_uri: String, auth_token: String, term: String, index: Option<u32>, count: Option<u32>) -> Result<music_services::SmapiSearchResult, String> {
    let client = reqwest::Client::new();
    music_services::search_music_service(&client, &service_uri, &auth_token, &term, index.unwrap_or(0), count.unwrap_or(100)).await
}
