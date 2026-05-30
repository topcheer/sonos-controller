use serde::{Deserialize, Serialize};
use reqwest::Client;

const AV_TRANSPORT: &str = "urn:schemas-upnp-org:service:AVTransport:1";
const RENDERING_CONTROL: &str = "urn:schemas-upnp-org:service:RenderingControl:1";
const CONTENT_DIRECTORY: &str = "urn:schemas-upnp-org:service:ContentDirectory:1";
const GROUP_MANAGEMENT: &str = "urn:schemas-upnp-org:service:GroupManagement:1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportInfo {
    pub state: String,
    pub status: String,
    pub speed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    pub track: u32,
    pub track_duration: String,
    pub track_uri: String,
    pub track_metadata: String,
    pub rel_time: String,
    pub abs_time: String,
    pub rel_count: i32,
    pub abs_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    pub nr_tracks: u32,
    pub media_duration: String,
    pub current_uri: String,
    pub current_uri_metadata: String,
    pub next_uri: String,
    pub next_uri_metadata: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayMode {
    pub play_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossfadeMode {
    pub crossfade: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqData {
    pub eq_type: String,
    pub value: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepTimer {
    pub remaining_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportSettings {
    pub play_mode: String,
    pub repeat: bool,
    pub shuffle: bool,
    pub crossfade: bool,
}

pub struct UpnpControl {
    client: Client,
    base_url: String,
}

impl UpnpControl {
    pub fn new(ip: &str) -> Self {
        Self {
            client: Client::builder().timeout(std::time::Duration::from_secs(5)).build().unwrap_or_default(),
            base_url: format!("http://{}:1400", ip),
        }
    }

    // ==================== AVTransport ====================

    pub async fn get_transport_info(&self) -> Result<TransportInfo, String> {
        let xml = self.soap("AVTransport", "GetTransportInfo", "<InstanceID>0</InstanceID>").await?;
        Ok(TransportInfo {
            state: extract(&xml, "CurrentTransportState").unwrap_or_default(),
            status: extract(&xml, "CurrentTransportStatus").unwrap_or_default(),
            speed: extract(&xml, "CurrentSpeed").unwrap_or_default(),
        })
    }

    pub async fn get_position_info(&self) -> Result<PositionInfo, String> {
        let xml = self.soap("AVTransport", "GetPositionInfo", "<InstanceID>0</InstanceID>").await?;
        Ok(PositionInfo {
            track: extract(&xml, "Track").unwrap_or_default().parse().unwrap_or(0),
            track_duration: extract(&xml, "TrackDuration").unwrap_or_default(),
            track_uri: extract(&xml, "TrackURI").unwrap_or_default(),
            track_metadata: extract(&xml, "TrackMetaData").unwrap_or_default(),
            rel_time: extract(&xml, "RelTime").unwrap_or_default(),
            abs_time: extract(&xml, "AbsTime").unwrap_or_default(),
            rel_count: extract(&xml, "RelCount").unwrap_or_default().parse().unwrap_or(-1),
            abs_count: extract(&xml, "AbsCount").unwrap_or_default().parse().unwrap_or(-1),
        })
    }

    pub async fn get_media_info(&self) -> Result<MediaInfo, String> {
        let xml = self.soap("AVTransport", "GetMediaInfo", "<InstanceID>0</InstanceID>").await?;
        Ok(MediaInfo {
            nr_tracks: extract(&xml, "NrTracks").unwrap_or_default().parse().unwrap_or(0),
            media_duration: extract(&xml, "MediaDuration").unwrap_or_default(),
            current_uri: extract(&xml, "CurrentURI").unwrap_or_default(),
            current_uri_metadata: extract(&xml, "CurrentURIMetaData").unwrap_or_default(),
            next_uri: extract(&xml, "NextURI").unwrap_or_default(),
            next_uri_metadata: extract(&xml, "NextURIMetaData").unwrap_or_default(),
        })
    }

    pub async fn play(&self) -> Result<(), String> { self.soap("AVTransport", "Play", "<InstanceID>0</InstanceID><Speed>1</Speed>").await?; Ok(()) }
    pub async fn pause(&self) -> Result<(), String> { self.soap("AVTransport", "Pause", "<InstanceID>0</InstanceID>").await?; Ok(()) }
    pub async fn stop(&self) -> Result<(), String> { self.soap("AVTransport", "Stop", "<InstanceID>0</InstanceID>").await?; Ok(()) }
    pub async fn next(&self) -> Result<(), String> { self.soap("AVTransport", "Next", "<InstanceID>0</InstanceID>").await?; Ok(()) }
    pub async fn previous(&self) -> Result<(), String> { self.soap("AVTransport", "Previous", "<InstanceID>0</InstanceID>").await?; Ok(()) }

    pub async fn seek(&self, target: &str) -> Result<(), String> {
        self.soap("AVTransport", "Seek", &format!("<InstanceID>0</InstanceID><Unit>REL_TIME</Unit><Target>{}</Target>", target)).await?;
        Ok(())
    }

    pub async fn seek_track(&self, track: u32) -> Result<(), String> {
        self.soap("AVTransport", "Seek", &format!("<InstanceID>0</InstanceID><Unit>TRACK_NR</Unit><Target>{}</Target>", track)).await?;
        Ok(())
    }

    // PlayMode: NORMAL, REPEAT_ALL, SHUFFLE_NOREPEAT, SHUFFLE
    pub async fn get_play_mode(&self) -> Result<PlayMode, String> {
        let xml = self.soap("AVTransport", "GetTransportSettings", "<InstanceID>0</InstanceID>").await?;
        Ok(PlayMode { play_mode: extract(&xml, "PlayMode").unwrap_or_default() })
    }

    pub async fn set_play_mode(&self, mode: &str) -> Result<(), String> {
        self.soap("AVTransport", "SetPlayMode", &format!("<InstanceID>0</InstanceID><NewPlayMode>{}</NewPlayMode>", mode)).await?;
        Ok(())
    }

    pub async fn get_crossfade(&self) -> Result<CrossfadeMode, String> {
        let xml = self.soap("AVTransport", "GetCrossfadeMode", "<InstanceID>0</InstanceID>").await?;
        Ok(CrossfadeMode { crossfade: extract(&xml, "CrossfadeMode").unwrap_or_default() == "1" })
    }

    pub async fn set_crossfade(&self, on: bool) -> Result<(), String> {
        self.soap("AVTransport", "SetCrossfadeMode", &format!("<InstanceID>0</InstanceID><CrossfadeMode>{}</CrossfadeMode>", if on { 1 } else { 0 })).await?;
        Ok(())
    }

    pub async fn get_transport_settings(&self) -> Result<TransportSettings, String> {
        let xml = self.soap("AVTransport", "GetTransportSettings", "<InstanceID>0</InstanceID>").await?;
        let mode = extract(&xml, "PlayMode").unwrap_or_default();
        Ok(TransportSettings {
            repeat: mode.contains("REPEAT"),
            shuffle: mode.contains("SHUFFLE"),
            crossfade: false, // need separate call
            play_mode: mode,
        })
    }

    pub async fn set_av_transport_uri(&self, uri: &str, metadata: &str) -> Result<(), String> {
        let esc_uri = html_escape(uri);
        let esc_meta = html_escape(metadata);
        self.soap("AVTransport", "SetAVTransportURI", &format!(
            "<InstanceID>0</InstanceID><CurrentURI>{}</CurrentURI><CurrentURIMetaData>{}</CurrentURIMetaData>", esc_uri, esc_meta
        )).await?;
        Ok(())
    }

    // Sleep timer
    pub async fn get_sleep_timer(&self) -> Result<SleepTimer, String> {
        let xml = self.soap("AVTransport", "GetRemainingSleepTimerDuration", "<InstanceID>0</InstanceID>").await?;
        Ok(SleepTimer { remaining_time: extract(&xml, "RemainingSleepTimerDuration").unwrap_or_default() })
    }

    pub async fn set_sleep_timer(&self, seconds: u32) -> Result<(), String> {
        self.soap("AVTransport", "ConfigureSleepTimer", &format!(
            "<InstanceID>0</InstanceID><NewSleepTimerDuration>{:02}:{:02}:{:02}</NewSleepTimerDuration>",
            seconds / 3600, (seconds % 3600) / 60, seconds % 60
        )).await?;
        Ok(())
    }

    // ==================== RenderingControl ====================

    pub async fn get_volume(&self) -> Result<u8, String> {
        let xml = self.soap("RenderingControl", "GetVolume", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        extract(&xml, "CurrentVolume").unwrap_or_default().parse::<u8>().map_err(|e: std::num::ParseIntError| e.to_string())
    }

    pub async fn set_volume(&self, volume: u8) -> Result<(), String> {
        self.soap("RenderingControl", "SetVolume", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredVolume>{}</DesiredVolume>", volume)).await?;
        Ok(())
    }

    pub async fn get_mute(&self) -> Result<bool, String> {
        let xml = self.soap("RenderingControl", "GetMute", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        extract(&xml, "CurrentMute").unwrap_or_default().parse::<u8>().map(|v| v == 1).map_err(|e: std::num::ParseIntError| e.to_string())
    }

    pub async fn set_mute(&self, mute: bool) -> Result<(), String> {
        self.soap("RenderingControl", "SetMute", &format!("<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredMute>{}</DesiredMute>", if mute { 1 } else { 0 })).await?;
        Ok(())
    }

    // EQ bands: DialogLevel, SurroundLevel, MusicSurroundLevel, NightMode, SubGain, HeightChannelLevel, CenterLevel, RearLevel
    pub async fn get_eq(&self, eq_type: &str) -> Result<EqData, String> {
        let xml = self.soap("RenderingControl", "GetEQ", &format!(
            "<InstanceID>0</InstanceID><EQType>{}</EQType>", eq_type
        )).await?;
        Ok(EqData {
            eq_type: eq_type.to_string(),
            value: extract(&xml, "CurrentValue").unwrap_or_default().parse().unwrap_or(0),
        })
    }

    pub async fn set_eq(&self, eq_type: &str, value: i16) -> Result<(), String> {
        self.soap("RenderingControl", "SetEQ", &format!(
            "<InstanceID>0</InstanceID><EQType>{}</EQType><DesiredValue>{}</DesiredValue>", eq_type, value
        )).await?;
        Ok(())
    }

    // Loudness
    pub async fn get_loudness(&self) -> Result<bool, String> {
        let xml = self.soap("RenderingControl", "GetLoudness", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        extract(&xml, "CurrentLoudness").unwrap_or_default().parse::<u8>().map(|v| v == 1).map_err(|e: std::num::ParseIntError| e.to_string())
    }

    pub async fn set_loudness(&self, on: bool) -> Result<(), String> {
        self.soap("RenderingControl", "SetLoudness", &format!(
            "<InstanceID>0</InstanceID><Channel>Master</Channel><DesiredLoudness>{}</DesiredLoudness>", if on { 1 } else { 0 }
        )).await?;
        Ok(())
    }

    // Bass/Treble (-10 to +10)
    pub async fn get_bass(&self) -> Result<i16, String> {
        let xml = self.soap("RenderingControl", "GetBass", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        extract(&xml, "CurrentBass").unwrap_or_default().parse().map_err(|e: std::num::ParseIntError| e.to_string())
    }

    pub async fn set_bass(&self, value: i16) -> Result<(), String> {
        self.soap("RenderingControl", "SetBass", &format!("<InstanceID>0</InstanceID><DesiredBass>{}</DesiredBass>", value)).await?;
        Ok(())
    }

    pub async fn get_treble(&self) -> Result<i16, String> {
        let xml = self.soap("RenderingControl", "GetTreble", "<InstanceID>0</InstanceID><Channel>Master</Channel>").await?;
        extract(&xml, "CurrentTreble").unwrap_or_default().parse().map_err(|e: std::num::ParseIntError| e.to_string())
    }

    pub async fn set_treble(&self, value: i16) -> Result<(), String> {
        self.soap("RenderingControl", "SetTreble", &format!("<InstanceID>0</InstanceID><DesiredTreble>{}</DesiredTreble>", value)).await?;
        Ok(())
    }

    // ==================== Group Management ====================

    // Join a group by routing audio through coordinator
    pub async fn join_group(&self, coordinator_ip: &str) -> Result<(), String> {
        // Get the coordinator's UUID
        let url = format!("http://{}:1400/xml/device_description.xml", coordinator_ip);
        let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
        let xml = resp.text().await.map_err(|e| e.to_string())?;
        let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;
        let uid = doc.descendants()
            .find(|n| n.has_tag_name("UDN"))
            .and_then(|n| n.text().map(|t| t.to_string()))
            .unwrap_or_default();

        // Set transport URI to coordinator's group
        self.set_av_transport_uri(&format!("x-rincon:{}", uid.replace("uuid:", "")), "").await?;
        Ok(())
    }

    // Leave group (become standalone)
    pub async fn leave_group(&self) -> Result<(), String> {
        self.set_av_transport_uri("", "").await?;
        Ok(())
    }

    // ==================== SOAP ====================

    async fn soap(&self, service: &str, action: &str, args: &str) -> Result<String, String> {
        let (service_type, control_url) = match service {
            "AVTransport" => (AV_TRANSPORT, "/MediaRenderer/AVTransport/Control"),
            "RenderingControl" => (RENDERING_CONTROL, "/MediaRenderer/RenderingControl/Control"),
            "ContentDirectory" => (CONTENT_DIRECTORY, "/MediaServer/ContentDirectory/Control"),
            "GroupManagement" => (GROUP_MANAGEMENT, "/MediaRenderer/GroupManagement/Control"),
            _ => return Err(format!("Unknown service: {}", service)),
        };

        let body = format!(
            r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:{action} xmlns:u="{service_type}">
      {args}
    </u:{action}>
  </s:Body>
</s:Envelope>"#,
        );

        let url = format!("{}{}", self.base_url, control_url);
        let soap_action = format!("{}#{}", service_type, action);

        let resp = self.client.post(&url)
            .header("SOAPAction", format!("\"{}\"", soap_action))
            .header("Content-Type", "text/xml; charset=utf-8")
            .body(body)
            .send().await.map_err(|e| e.to_string())?;

        let text = resp.text().await.map_err(|e| e.to_string())?;

        if text.contains("UPnPError") {
            let code = extract(&text, "errorCode").unwrap_or_default();
            let desc = extract(&text, "errorDescription").unwrap_or_default();
            return Err(format!("UPnP Error {}: {}", code, desc));
        }

        Ok(text)
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

/// Low-level SOAP call to a Sonos device (for services not covered by SonosDevice)
pub async fn soap_raw(client: &Client, ip: &str, service: &str, service_type: &str, action: &str, args: &str) -> Result<String, String> {
    let control_url = match service {
        "AVTransport" => "/MediaRenderer/AVTransport/Control",
        "RenderingControl" => "/MediaRenderer/RenderingControl/Control",
        "ContentDirectory" => "/MediaServer/ContentDirectory/Control",
        "GroupManagement" => "/MediaRenderer/GroupManagement/Control",
        "MusicServices" => "/MusicServices/Control",
        "DeviceProperties" => "/DeviceProperties/Control",
        "ZoneGroupTopology" => "/ZoneGroupTopology/Control",
        "AlarmClock" => "/AlarmClock/Control",
        "SystemProperties" => "/SystemProperties/Control",
        _ => "/Unknown/Control",
    };
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:{action} xmlns:u="{service_type}">
      {args}
    </u:{action}>
  </s:Body>
</s:Envelope>"#,
    );
    let url = format!("http://{}:1400{}", ip, control_url);
    let soap_action = format!("{}#{}", service_type, action);
    let resp = client.post(&url)
        .header("SOAPAction", format!("\"{}\"", soap_action))
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send().await.map_err(|e| e.to_string())?;
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if text.contains("UPnPError") {
        let code = extract(&text, "errorCode").unwrap_or_default();
        let desc = extract(&text, "errorDescription").unwrap_or_default();
        return Err(format!("UPnP Error {}: {}", code, desc));
    }
    Ok(text)
}

pub fn extract(xml: &str, tag: &str) -> Option<String> {
    // Try normal XML
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    if let Some(start) = xml.find(&open) {
        let cs = start + open.len();
        if let Some(end) = xml[cs..].find(&close) {
            return Some(xml[cs..cs + end].to_string());
        }
    }
    // Try HTML-encoded
    let open_e = format!("&lt;{}&gt;", tag);
    let close_e = format!("&lt;/{}&gt;", tag);
    if let Some(start) = xml.find(&open_e) {
        let cs = start + open_e.len();
        if let Some(end) = xml[cs..].find(&close_e) {
            let raw = &xml[cs..cs + end];
            return Some(raw.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\""));
        }
    }
    None
}
