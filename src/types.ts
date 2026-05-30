export interface SonosDevice { ip: string; port: number; name: string; model: string; model_number: string; serial: string; firmware: string; room_name: string; icon_url: string | null; uid: string; }
export interface ZoneGroup { coordinator_ip: string; coordinator_name: string; coordinator_uid: string; id: string; members: ZoneGroupMember[]; hidden_ips: string[]; }
export interface ZoneGroupMember { ip: string; name: string; room_name: string; uid: string; icon_url: string | null; is_coordinator: boolean; is_invisible: boolean; }
export interface TransportInfo { state: string; status: string; speed: string; }
export interface PositionInfo { track: number; track_duration: string; track_uri: string; track_metadata: string; rel_time: string; abs_time: string; rel_count: number; abs_count: number; }
export interface MediaInfo { nr_tracks: number; media_duration: string; current_uri: string; current_uri_metadata: string; next_uri: string; next_uri_metadata: string; }
export interface TrackInfo { title: string; artist: string; album: string; album_art_uri: string; duration: string; uri: string; }
export interface QueueItem { title: string; artist: string; album: string; album_art_uri: string; uri: string; duration: string; track_number: number; }
export interface BrowserItem { title: string; uri: string | null; object_id: string; is_container: boolean; album_art_uri: string | null; description: string | null; }
export interface PlayMode { play_mode: string; }
export interface CrossfadeMode { crossfade: boolean; }
export interface TransportSettings { play_mode: string; repeat: boolean; shuffle: boolean; crossfade: boolean; }
export interface EqData { eq_type: string; value: number; }
export interface SleepTimer { remaining_time: string; }

// Music Services (SMAPI)
export interface MusicService { id: number; name: string; uri: string; secure_uri: string; auth: string; version: string; container_type: string; }
export interface SmapiItem { id: string; title: string; item_type: string; artist: string | null; album: string | null; album_art_uri: string | null; uri: string | null; description: string | null; playback_metadata: string | null; }
export interface SmapiBrowseResult { items: SmapiItem[]; index: number; total: number; }
export interface SmapiSearchResult { items: SmapiItem[]; index: number; total: number; }
