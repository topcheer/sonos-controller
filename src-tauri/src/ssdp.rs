use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::Duration;

/// Sonos device discovered via SSDP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SonosDevice {
    pub ip: String,
    pub port: u16,
    pub name: String,
    pub model: String,
    pub model_number: String,
    pub serial: String,
    pub firmware: String,
    pub room_name: String,
    pub icon_url: Option<String>,
    pub uid: String,
}

/// Zone Group (a group of speakers playing together)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneGroup {
    pub coordinator_ip: String,
    pub coordinator_name: String,
    pub coordinator_uid: String,
    pub id: String,
    pub members: Vec<ZoneGroupMember>,
    /// IPs of invisible (stereo pair slave) members, not shown in members list
    pub hidden_ips: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZoneGroupMember {
    pub ip: String,
    pub name: String,
    pub room_name: String,
    pub uid: String,
    pub icon_url: Option<String>,
    pub is_coordinator: bool,
    pub is_invisible: bool,
}

/// Queue item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_art_uri: String,
    pub uri: String,
    pub duration: String,
    pub track_number: u32,
}

/// Browser item (from ContentDirectory)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserItem {
    pub title: String,
    pub uri: Option<String>,
    pub object_id: String,
    pub is_container: bool,
    pub album_art_uri: Option<String>,
    pub description: Option<String>,
}

/// Helper: get XML attribute with default
fn attr<'a>(node: &'a roxmltree::Node, name: &str) -> &'a str {
    node.attribute(name).unwrap_or("")
}

/// Discover Sonos devices on the local network using SSDP
pub async fn discover_sonos(timeout_secs: u64) -> Result<Vec<SonosDevice>, String> {
    log::info!("Starting SSDP discovery for Sonos devices...");

    let locations = collect_ssdp_responses(timeout_secs)?;
    log::info!("SSDP found {} unique locations", locations.len());

    if locations.is_empty() {
        return Ok(vec![]);
    }

    let mut devices = Vec::new();
    for location in &locations {
        match fetch_device_description(location).await {
            Ok(device) => {
                log::info!("Found device: {} ({})", device.room_name, device.ip);
                devices.push(device);
            }
            Err(e) => {
                log::warn!("Failed to fetch description from {}: {}", location, e);
            }
        }
    }

    let mut seen = HashMap::new();
    for d in devices {
        seen.entry(d.ip.clone()).or_insert(d);
    }

   let mut list: Vec<SonosDevice> = seen.into_values().collect();
   list.sort_by(|a, b| a.room_name.cmp(&b.room_name));
   Ok(list)
}

/// Get zone groups from a Sonos device
pub async fn get_zone_groups(ip: &str) -> Result<Vec<ZoneGroup>, String> {
    let url = format!("http://{}:1400/status/zp", ip);
    let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
    let xml = resp.text().await.map_err(|e| e.to_string())?;
    let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;

    let mut groups: Vec<ZoneGroup> = Vec::new();

    // Parse ZoneGroups from /status/zp
    for zgs in doc.descendants().filter(|n| n.has_tag_name("ZoneGroups")) {
        for zg in zgs.descendants().filter(|n| n.has_tag_name("ZoneGroup")) {
            let coordinator = attr(&zg, "Coordinator");
            let id = attr(&zg, "ID");

            let mut members = Vec::new();
            let mut hidden_ips: Vec<String> = Vec::new();
            let mut coord_ip = String::new();
            let mut coord_name = String::new();
            let mut coord_uid = String::new();

            for member in zg.descendants().filter(|n| n.has_tag_name("ZoneGroupMember")) {
                let location = attr(&member, "Location");
                let member_ip = if location.starts_with("http") {
                    url::Url::parse(location)
                        .map(|u| u.host_str().unwrap_or("").to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                let name = attr(&member, "ZoneGroupName").to_string();
                let room_name = attr(&member, "ZoneName").to_string();
                let uid = attr(&member, "UUID").to_string();
                let is_coord = attr(&member, "IsCoordinator") == "1";
                let is_invisible = attr(&member, "Invisible") == "1";

                if is_invisible {
                    hidden_ips.push(member_ip.clone());
                    // Invisible might still be coordinator (stereo pair master hidden)
                    if is_coord {
                        coord_ip = member_ip.clone();
                        coord_name = if name.is_empty() { room_name.clone() } else { name.clone() };
                        coord_uid = uid.clone();
                    }
                    continue;
                }

                // Extract icon URL
                let icon_url = member
                    .descendants()
                    .find(|n| n.has_tag_name("URL"))
                    .and_then(|n| n.text().map(|t| t.to_string()));

                if is_coord {
                    coord_ip = member_ip.clone();
                    coord_name = if name.is_empty() { room_name.clone() } else { name.clone() };
                    coord_uid = uid.clone();
                }

                members.push(ZoneGroupMember {
                    ip: member_ip,
                    name,
                    room_name,
                    uid,
                    icon_url,
                    is_coordinator: is_coord,
                    is_invisible: false,
                });
            }

            if !coord_ip.is_empty() {
                let display_name = if coord_name.is_empty() {
                    members.first().map(|m| m.room_name.clone()).unwrap_or_default()
                } else {
                    coord_name
                };
                groups.push(ZoneGroup {
                    coordinator_ip: coord_ip,
                    coordinator_name: display_name,
                    coordinator_uid: coord_uid,
                    id: id.to_string(),
                    members,
                    hidden_ips,
                });
            }
        }
    }

    // If /status/zp didn't return groups, try ZoneGroupTopology SOAP
    if groups.is_empty() {
        log::info!("/status/zp returned no groups, trying SOAP...");
        return get_zone_groups_soap(ip).await;
    }

    groups.sort_by(|a, b| a.coordinator_name.cmp(&b.coordinator_name));
    log::info!("Found {} zone groups", groups.len());
    Ok(groups)
}

/// Fallback: Get zone groups via UPnP SOAP
async fn get_zone_groups_soap(ip: &str) -> Result<Vec<ZoneGroup>, String> {
    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:GetZoneGroupState xmlns:u="urn:schemas-upnp-org:service:ZoneGroupTopology:1">
      <InstanceID>0</InstanceID>
    </u:GetZoneGroupState>
  </s:Body>
</s:Envelope>"#;

    let url = format!("http://{}:1400/ZoneGroupTopology/Control", ip);
    let resp = reqwest::Client::new()
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:ZoneGroupTopology:1#GetZoneGroupState\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = resp.text().await.map_err(|e| e.to_string())?;

    // Extract the ZoneGroupState from SOAP response
    let state_xml = if let Some(start) = text.find("&lt;ZoneGroups") {
        let unescaped = text
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
            .replace("&quot;", "\"");
        unescaped
    } else {
        text
    };

    let doc = roxmltree::Document::parse(&state_xml).map_err(|e| e.to_string())?;
    let mut groups: Vec<ZoneGroup> = Vec::new();

    for zg in doc.descendants().filter(|n| n.has_tag_name("ZoneGroup")) {
        let coordinator = attr(&zg, "Coordinator");
        let id = attr(&zg, "ID");

        let mut members = Vec::new();
        let mut hidden_ips: Vec<String> = Vec::new();
        let mut coord_ip = String::new();
        let mut coord_name = String::new();
        let mut coord_uid = String::new();

        for member in zg.descendants().filter(|n| n.has_tag_name("ZoneGroupMember")) {
            let location = attr(&member, "Location");
            let member_ip = if location.starts_with("http") {
                url::Url::parse(location)
                    .map(|u| u.host_str().unwrap_or("").to_string())
                    .unwrap_or_default()
            } else {
                String::new()
            };

            let name = attr(&member, "ZoneGroupName").to_string();
            let room_name = attr(&member, "ZoneName").to_string();
            let uid = attr(&member, "UUID").to_string();
            let is_coord = uid == coordinator;
            let is_invisible = attr(&member, "Invisible") == "1";

            if is_invisible {
                hidden_ips.push(member_ip.clone());
                if is_coord {
                    coord_ip = member_ip.clone();
                    coord_name = if name.is_empty() { room_name.clone() } else { name.clone() };
                    coord_uid = uid.clone();
                }
                continue;
            }

            if is_coord {
                coord_ip = member_ip.clone();
                coord_name = if name.is_empty() { room_name.clone() } else { name.clone() };
                coord_uid = uid.clone();
            }

            members.push(ZoneGroupMember {
                ip: member_ip,
                name,
                room_name,
                uid,
                icon_url: None,
                is_coordinator: is_coord,
                is_invisible: false,
            });
        }

        if !coord_ip.is_empty() {
            let display_name = if coord_name.is_empty() {
                members.first().map(|m| m.room_name.clone()).unwrap_or_default()
            } else {
                coord_name
            };
            groups.push(ZoneGroup {
                coordinator_ip: coord_ip,
                coordinator_name: display_name,
                coordinator_uid: coord_uid,
                id: id.to_string(),
                members,
                hidden_ips,
            });
        }
    }

    groups.sort_by(|a, b| a.coordinator_name.cmp(&b.coordinator_name));
    log::info!("SOAP: Found {} zone groups", groups.len());
    Ok(groups)
}

/// Get queue from a Sonos device
pub async fn get_queue(ip: &str, start: u32, limit: u32) -> Result<Vec<QueueItem>, String> {
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>Q:0</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>dc:title,dc:creator,upnp:album,upnp:albumArtURI,res</Filter>
      <StartingIndex>{}</StartingIndex>
      <RequestedCount>{}</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#,
        start, limit
    );

    let url = format!("http://{}:1400/MediaServer/ContentDirectory/Control", ip);
    let resp = reqwest::Client::new()
        .post(&url)
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\"",
        )
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = resp.text().await.map_err(|e| e.to_string())?;
    let unescaped = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"");

    parse_didl_queue(&unescaped)
}

/// Browse ContentDirectory
pub async fn browse_directory(
    ip: &str,
    object_id: &str,
    start: u32,
    limit: u32,
) -> Result<Vec<BrowserItem>, String> {
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Browse xmlns:u="urn:schemas-upnp-org:service:ContentDirectory:1">
      <ObjectID>{}</ObjectID>
      <BrowseFlag>BrowseDirectChildren</BrowseFlag>
      <Filter>dc:title,dc:creator,upnp:album,upnp:albumArtURI,res</Filter>
      <StartingIndex>{}</StartingIndex>
      <RequestedCount>{}</RequestedCount>
      <SortCriteria></SortCriteria>
    </u:Browse>
  </s:Body>
</s:Envelope>"#,
        object_id, start, limit
    );

    let url = format!("http://{}:1400/MediaServer/ContentDirectory/Control", ip);
    let resp = reqwest::Client::new()
        .post(&url)
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:ContentDirectory:1#Browse\"",
        )
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let text = resp.text().await.map_err(|e| e.to_string())?;
    let unescaped = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
        .replace("&quot;", "\"");

    parse_didl_browse(&unescaped)
}

/// Play a specific URI (for queue items, favorites, etc.)
pub async fn play_uri(ip: &str, uri: &str, metadata: &str) -> Result<(), String> {
    let body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <CurrentURI>{}</CurrentURI>
      <CurrentURIMetaData>{}</CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"#,
        html_escape(uri),
        html_escape(metadata),
    );

    let url = format!("http://{}:1400/MediaRenderer/AVTransport/Control", ip);
    reqwest::Client::new()
        .post(&url)
        .header(
            "SOAPAction",
            "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"",
        )
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // Start playback
    let play_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </u:Play>
  </s:Body>
</s:Envelope>"#;

    let url = format!("http://{}:1400/MediaRenderer/AVTransport/Control", ip);
    reqwest::Client::new()
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Play\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(play_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Play from queue by track number
pub async fn play_from_queue(ip: &str, track_index: u32) -> Result<(), String> {
    let url = format!("http://{}:1400/MediaRenderer/AVTransport/Control", ip);
    let client = reqwest::Client::new();

    // SetAVTransportURI to queue — may fail if already on queue, that's OK
    let body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <CurrentURI>x-rincon-queue:Q:0#0</CurrentURI>
      <CurrentURIMetaData></CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"#;

    let _ = client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(body)
        .send()
        .await; // Ignore error — already on queue is fine

    // Seek to the track
    let seek_body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Seek xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Unit>TRACK_NR</Unit>
      <Target>{}</Target>
    </u:Seek>
  </s:Body>
</s:Envelope>"#,
        track_index
    );

    client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Seek\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(seek_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // Play
    let play_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </u:Play>
  </s:Body>
</s:Envelope>"#;

    client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Play\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(play_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

/// Add a URI to the queue and start playing it
pub async fn add_to_queue_and_play(ip: &str, uri: &str, metadata: &str) -> Result<(), String> {
    let url = format!("http://{}:1400/MediaRenderer/AVTransport/Control", ip);
    let client = reqwest::Client::new();

    // Step 1: Add URI to end of queue
    let escaped_uri = html_escape(uri);
    let escaped_meta = html_escape(metadata);
    let add_body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:AddURIToQueue xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <EnqueuedURI>{escaped_uri}</EnqueuedURI>
      <EnqueuedURIMetaData>{escaped_meta}</EnqueuedURIMetaData>
      <DesiredFirstTrackNumberEnqueued>0</DesiredFirstTrackNumberEnqueued>
      <EnqueueAsNext>1</EnqueueAsNext>
    </u:AddURIToQueue>
  </s:Body>
</s:Envelope>"#
    );

    let resp = client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#AddURIToQueue\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(add_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let resp_text = resp.text().await.map_err(|e| e.to_string())?;

    // Extract the track number that was assigned
    let track_num = regex::Regex::new(r"<FirstTrackNumberEnqueued>(\d+)</FirstTrackNumberEnqueued>")
        .unwrap()
        .captures(&resp_text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "0".to_string());

    // Step 2: SetAVTransportURI to queue
    let set_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:SetAVTransportURI xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <CurrentURI>x-rincon-queue:Q:0#0</CurrentURI>
      <CurrentURIMetaData></CurrentURIMetaData>
    </u:SetAVTransportURI>
  </s:Body>
</s:Envelope>"#;

    let _ = client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#SetAVTransportURI\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(set_body)
        .send()
        .await;

    // Step 3: Seek to the added track
    let seek_body = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Seek xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Unit>TRACK_NR</Unit>
      <Target>{track_num}</Target>
    </u:Seek>
  </s:Body>
</s:Envelope>"#
    );

    client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Seek\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(seek_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    // Step 4: Play
    let play_body = r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" s:encodingStyle="http://schemas.xmlsoap.org/soap/encoding/">
  <s:Body>
    <u:Play xmlns:u="urn:schemas-upnp-org:service:AVTransport:1">
      <InstanceID>0</InstanceID>
      <Speed>1</Speed>
    </u:Play>
  </s:Body>
</s:Envelope>"#;

    client
        .post(&url)
        .header("SOAPAction", "\"urn:schemas-upnp-org:service:AVTransport:1#Play\"")
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(play_body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parse_didl_queue(xml: &str) -> Result<Vec<QueueItem>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let mut items = Vec::new();
    let mut track_num = 0u32;

    for item in doc.descendants().filter(|n| n.has_tag_name("item")) {
        track_num += 1;
        let get_text = |tag: &str| -> String {
            item.descendants()
                .find(|n| n.has_tag_name(tag))
                .and_then(|n| n.text().map(|t| t.to_string()))
                .unwrap_or_default()
        };

        let mut art = get_text("albumArtURI");
        if art.starts_with('/') {
            art = String::new(); // Will be prefixed by frontend
        }

        items.push(QueueItem {
            title: get_text("title"),
            artist: get_text("creator"),
            album: get_text("album"),
            album_art_uri: art,
            uri: get_text("res"),
            duration: get_text("res")
                .split_once("duration=")
                .and_then(|(_, rest)| rest.split_once('"').and_then(|(_, r)| r.split_once('"')))
                .map(|(d, _)| d.to_string())
                .unwrap_or_default(),
            track_number: track_num,
        });
    }

    Ok(items)
}

fn parse_didl_browse(xml: &str) -> Result<Vec<BrowserItem>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let mut items = Vec::new();

    for node in doc.descendants().filter(|n| {
        n.has_tag_name("container") || n.has_tag_name("item")
    }) {
        let is_container = node.has_tag_name("container");
        let get_text = |tag: &str| -> String {
            node.descendants()
                .find(|n| n.has_tag_name(tag))
                .and_then(|n| n.text().map(|t| t.to_string()))
                .unwrap_or_default()
        };

        let object_id = attr(&node, "id").to_string();

        items.push(BrowserItem {
            title: get_text("title"),
            uri: if is_container {
                None
            } else {
                Some(get_text("res"))
            },
            object_id,
            is_container,
            album_art_uri: Some(get_text("albumArtURI")).filter(|u| !u.is_empty()),
            description: Some(get_text("description")).filter(|u| !u.is_empty()),
        });
    }

    Ok(items)
}

fn collect_ssdp_responses(timeout_secs: u64) -> Result<Vec<String>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;
    socket
        .set_multicast_ttl_v4(2)
        .map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|e| e.to_string())?;

    let msearch = "M-SEARCH * HTTP/1.1\r\n\
         Host: 239.255.255.250:1900\r\n\
         Man: \"ssdp:discover\"\r\n\
         MX: 3\r\n\
         ST: urn:schemas-upnp-org:device:ZonePlayer:1\r\n\r\n";

    let multicast_addr = SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900);

    for _ in 0..3 {
        socket
            .send_to(msearch.as_bytes(), multicast_addr)
            .map_err(|e| e.to_string())?;
    }

    let mut locations: Vec<String> = Vec::new();
    let mut seen: HashMap<String, bool> = HashMap::new();
    let start = std::time::Instant::now();
    let deadline = Duration::from_secs(timeout_secs);

    while start.elapsed() < deadline {
        let mut buf = [0u8; 4096];
        match socket.recv_from(&mut buf) {
            Ok((len, _addr)) => {
                let response = String::from_utf8_lossy(&buf[..len]);
                if let Some(location) = extract_header(&response, "LOCATION") {
                    if !seen.contains_key(&location) {
                        log::info!("SSDP response from: {}", location);
                        seen.insert(location.clone(), true);
                        locations.push(location);
                    }
                }
            }
            Err(_) => {
                if !locations.is_empty() {
                    break;
                }
            }
        }
    }

    Ok(locations)
}

fn extract_header(response: &str, header: &str) -> Option<String> {
    let prefix = format!("{}:", header.to_lowercase());
    for line in response.lines() {
        let lower = line.to_lowercase();
        if lower.starts_with(&prefix) {
            let value = line.splitn(2, ':').nth(1).unwrap_or("").trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

async fn fetch_device_description(location: &str) -> Result<SonosDevice, String> {
    let resp = reqwest::get(location)
        .await
        .map_err(|e| e.to_string())?;
    let xml = resp.text().await.map_err(|e| e.to_string())?;
    let doc = roxmltree::Document::parse(&xml).map_err(|e| e.to_string())?;

    let url = url::Url::parse(location).map_err(|e| e.to_string())?;
    let ip = url.host_str().unwrap_or("unknown").to_string();
    let port = url.port().unwrap_or(1400);

    let find_text = |path: &[&str]| -> String {
        let mut current: Vec<roxmltree::Node> = doc.root().children().collect();
        for (i, tag) in path.iter().enumerate() {
            let found = current.iter().find(|n| n.has_tag_name(*tag));
            match found {
                Some(node) => {
                    if i == path.len() - 1 {
                        return node.text().unwrap_or("").to_string();
                    }
                    current = node.children().collect();
                }
                None => return String::new(),
            }
        }
        String::new()
    };

    let icon_url = doc
        .descendants()
        .find(|n| {
            n.has_tag_name("mimetype") && n.text().unwrap_or("").starts_with("image/")
        })
        .and_then(|mime_node| {
            mime_node.parent().and_then(|icon| {
                icon.descendants()
                    .find(|n| n.has_tag_name("url"))
                    .and_then(|n| n.text().map(|t| t.to_string()))
            })
        });

    Ok(SonosDevice {
        ip,
        port,
        name: find_text(&["root", "device", "friendlyName"]),
        model: find_text(&["root", "device", "modelName"]),
        model_number: find_text(&["root", "device", "modelNumber"]),
        serial: find_text(&["root", "device", "serialNum"]),
        firmware: find_text(&["root", "device", "softwareVersion"]),
        room_name: find_text(&["root", "device", "roomName"]),
        icon_url,
        uid: find_text(&["root", "device", "UDN"]).replace("uuid:", ""),
    })
}
