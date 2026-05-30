use reqwest::Client;
use roxmltree::Document;
use serde::{Deserialize, Serialize};

// ── Data types ──────────────────────────────────────────────────

/// Music service info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicService {
    pub id: u32,
    pub name: String,
    pub uri: String,
    pub secure_uri: String,
    pub auth: String,
    pub version: String,
    pub container_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmapiItem {
    pub id: String,
    pub title: String,
    pub item_type: String, // "container" | "track"
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_art_uri: Option<String>,
    pub uri: Option<String>,
    pub description: Option<String>,
    /// Full DIDL-Lite metadata needed for SetAVTransportURI playback
    pub playback_metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmapiBrowseResult {
    pub items: Vec<SmapiItem>,
    pub index: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmapiSearchResult {
    pub items: Vec<SmapiItem>,
    pub index: u32,
    pub total: u32,
}

// ── Helper ──────────────────────────────────────────────────────

fn attr<'a>(node: &'a roxmltree::Node, name: &str) -> &'a str {
    node.attribute(name).unwrap_or("")
}

fn text_of(node: &roxmltree::Node, tag: &str) -> Option<String> {
    node.descendants().find(|n| n.has_tag_name(tag)).and_then(|n| n.text().map(|t| t.to_string()))
}

fn smapi_envelope(body: &str, credentials: Option<&str>) -> String {
    let creds = credentials.unwrap_or("<ns:credentials></ns:credentials>");
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/" xmlns:ns="http://www.sonos.com/Services/1.1">
  <s:Header>{creds}</s:Header>
  <s:Body>{body}</s:Body>
</s:Envelope>"#
    )
}

async fn smapi_request(client: &Client, uri: &str, action: &str, body: &str, credentials: Option<&str>) -> Result<String, String> {
    let envelope = smapi_envelope(body, credentials);
    let resp = client
        .post(uri)
        .header("SOAPAction", format!("http://www.sonos.com/Services/1.1#{action}"))
        .header("Content-Type", "text/xml; charset=utf-8")
        .body(envelope)
        .send()
        .await
        .map_err(|e| format!("SMAPI request failed: {e}"))?;
    let text = resp.text().await.map_err(|e| format!("SMAPI read failed: {e}"))?;
    // Some services return JSON errors instead of XML (e.g. QQ Music SMAPI crashes)
    if text.trim_start().starts_with('{') {
        if let Some(msg) = text.find("errorMessage") {
            return Err(format!("Service error: {}", &text[msg..text.len().min(msg+200)]));
        }
        return Err(format!("Non-XML response: {}", &text[..text.len().min(200)]));
    }
    // Check for SOAP fault
    if text.contains("<faultstring>") {
        if let Some(code_start) = text.find("<faultcode>") {
            let code_end = text.find("</faultcode>").unwrap_or(code_start + 50);
            let fault_start = text.find("<faultstring>").unwrap_or(code_start);
            let fault_end = text.find("</faultstring>").unwrap_or(fault_start + 100);
            return Err(format!("SOAP fault: {} - {}", 
                &text[code_start+12..code_end],
                &text[fault_start+13..fault_end]));
        }
    }
    Ok(text)
}

fn parse_media_collection(xml: &str) -> Vec<SmapiItem> {
    let doc = match Document::parse(xml) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    let mut items = Vec::new();
    for node in doc.descendants() {
        let tag = node.tag_name().name();
        if tag == "mediaCollection" || tag == "mediaMetadata" {
            let item_type = if tag == "mediaCollection" { "container" } else { "track" };
            let id = attr(&node, "id").to_string();
            let title = text_of(&node, "title").unwrap_or_default();
            let artist = text_of(&node, "artist");
            let album = text_of(&node, "album");
            let album_art_uri = text_of(&node, "albumArtURI");
            let uri = text_of(&node, "uri");
            let description = text_of(&node, "description");
            items.push(SmapiItem { id, title, item_type: item_type.to_string(), artist, album, album_art_uri, uri, description, playback_metadata: None });
        }
    }
    items
}

fn parse_browse_response(xml: &str) -> SmapiBrowseResult {
    let doc = match Document::parse(xml) {
        Ok(d) => d,
        Err(_) => return SmapiBrowseResult { items: Vec::new(), index: 0, total: 0 },
    };
    let mut index = 0u32;
    let mut total = 0u32;
    for node in doc.descendants() {
        if node.has_tag_name("index") {
            if let Some(t) = node.text() { index = t.parse().unwrap_or(0); }
        }
        if node.has_tag_name("total") {
            if let Some(t) = node.text() { total = t.parse().unwrap_or(0); }
        }
    }
    let items = parse_media_collection(xml);
    SmapiBrowseResult { items, index, total }
}

fn parse_search_response(xml: &str) -> SmapiSearchResult {
    let doc = match Document::parse(xml) {
        Ok(d) => d,
        Err(_) => return SmapiSearchResult { items: Vec::new(), index: 0, total: 0 },
    };
    let mut index = 0u32;
    let mut total = 0u32;
    for node in doc.descendants() {
        if node.has_tag_name("index") { if let Some(t) = node.text() { index = t.parse().unwrap_or(0); } }
        if node.has_tag_name("total") { if let Some(t) = node.text() { total = t.parse().unwrap_or(0); } }
    }
    let items = parse_media_collection(xml);
    SmapiSearchResult { items, index, total }
}

/// Build credentials header. If auth_token is empty, send device-only credentials (anonymous).
/// If auth_token is present, include loginToken.
fn make_credentials(auth_token: &str) -> String {
    if auth_token.is_empty() {
        // Anonymous / no-auth: just device info, no loginToken
        "<ns:credentials><ns:deviceId>RINCON_SonosController</ns:deviceId><ns:deviceProvider>Sonos</ns:deviceProvider></ns:credentials>".to_string()
    } else {
        format!("<ns:credentials><ns:deviceId>RINCON_SonosController</ns:deviceId><ns:deviceProvider>Sonos</ns:deviceProvider><ns:loginToken><ns:token>{auth_token}</ns:token><ns:key></ns:key><ns:householdId></ns:householdId></ns:loginToken></ns:credentials>")
    }
}

// ── Public API ──────────────────────────────────────────────────

/// List available music services from a Sonos device
pub async fn list_music_services(client: &Client, ip: &str) -> Result<Vec<MusicService>, String> {
    let body = r#"<u:ListAvailableServices xmlns:u="urn:schemas-upnp-org:service:MusicServices:1"></u:ListAvailableServices>"#;
    let resp = crate::upnp::soap_raw(client, ip, "MusicServices", "urn:schemas-upnp-org:service:MusicServices:1", "ListAvailableServices", body).await?;
    
    // Parse AvailableServiceTypeList to find which services have accounts bound
    let mut configured_sids = std::collections::HashSet::new();
    if let Some(type_list) = crate::upnp::extract(&resp, "AvailableServiceTypeList") {
        for encoded in type_list.split(',') {
            if let Ok(val) = encoded.trim().parse::<u32>() {
                configured_sids.insert(val >> 8);
            }
        }
    }

    // The response has HTML-escaped XML inside AvailableServiceDescriptorList
    let escaped_xml = crate::upnp::extract(&resp, "AvailableServiceDescriptorList")
        .ok_or("No AvailableServiceDescriptorList in response")?;
    
    let decoded = escaped_xml
        .replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&");

    let doc = Document::parse(&decoded).map_err(|e| format!("Parse services XML error: {e}"))?;
    let mut services = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("Service")) {
        let id: u32 = attr(&node, "Id").parse().unwrap_or(0);
        let name = attr(&node, "Name").to_string();
        let uri = attr(&node, "Uri").to_string();
        let secure_uri = attr(&node, "SecureUri").to_string();
        let version = attr(&node, "Version").to_string();
        let container_type = attr(&node, "ContainerType").to_string();
        let auth = node.descendants()
            .find(|n| n.has_tag_name("Policy"))
            .map(|n| attr(&n, "Auth").to_string())
            .unwrap_or_default();
        // Only include services that appear in the type list (bound to the system)
        if id > 0 && !name.is_empty() && configured_sids.contains(&id) {
            services.push(MusicService { id, name, uri, secure_uri, auth, version, container_type });
        }
    }
    services.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(services)
}

/// Get household ID from Sonos device
pub async fn get_household_id(client: &Client, ip: &str) -> Result<String, String> {
    let body = r#"<u:GetHouseholdID xmlns:u="urn:schemas-upnp-org:service:DeviceProperties:1"></u:GetHouseholdID>"#;
    let resp = crate::upnp::soap_raw(client, ip, "DeviceProperties", "urn:schemas-upnp-org:service:DeviceProperties:1", "GetHouseholdID", body).await?;
    let doc = Document::parse(&resp).map_err(|e| format!("Parse error: {e}"))?;
    for node in doc.descendants() {
        if node.has_tag_name("CurrentHouseholdID") {
            if let Some(t) = node.text() { return Ok(t.to_string()); }
        }
    }
    Err("HouseholdID not found".to_string())
}

/// Get AppLink for a music service (used for AppLink auth flow)
pub async fn get_app_link(client: &Client, service_uri: &str, household_id: &str) -> Result<String, String> {
    let body = format!(
        "<ns:getAppLink><ns:householdId>{household_id}</ns:householdId></ns:getAppLink>"
    );
    let creds = "<ns:credentials></ns:credentials>";
    let resp = smapi_request(client, service_uri, "getAppLink", &body, Some(creds)).await?;
    let doc = Document::parse(&resp).map_err(|e| format!("Parse error: {e}"))?;
    // Return the raw response for the frontend to parse the link
    Ok(resp)
}

/// Get auth token after user completes AppLink flow
pub async fn get_device_auth_token(client: &Client, service_uri: &str, household_id: &str, link_code: &str, link_device_id: &str) -> Result<String, String> {
    let body = format!(
        "<ns:getDeviceAuthToken><ns:householdId>{household_id}</ns:householdId><ns:linkCode>{link_code}</ns:linkCode><ns:linkDeviceId>{link_device_id}</ns:linkDeviceId></ns:getDeviceAuthToken>"
    );
    let creds = "<ns:credentials></ns:credentials>";
    let resp = smapi_request(client, service_uri, "getDeviceAuthToken", &body, Some(creds)).await?;
    let doc = Document::parse(&resp).map_err(|e| format!("Parse error: {e}"))?;
    for node in doc.descendants() {
        if node.has_tag_name("authToken") {
            if let Some(t) = node.text() { return Ok(t.to_string()); }
        }
        // Also check for private key
        if node.has_tag_name("privateKey") {
            // store this too if needed
        }
    }
    // If no authToken element, try returning the whole response
    Err(format!("No authToken in response: {}", &resp[..resp.len().min(200)]))
}

/// Browse a music service using SMAPI getMetadata
pub async fn browse_music_service(client: &Client, service_uri: &str, auth_token: &str, parent_id: &str, index: u32, count: u32) -> Result<SmapiBrowseResult, String> {
    let body = format!(
        "<ns:getMetadata><ns:id>{parent_id}</ns:id><ns:index>{index}</ns:index><ns:count>{count}</ns:count></ns:getMetadata>"
    );
    let creds = make_credentials(auth_token);
    let resp = smapi_request(client, service_uri, "getMetadata", &body, Some(&creds)).await?;
    Ok(parse_browse_response(&resp))
}

/// Search a music service using SMAPI search
pub async fn search_music_service(client: &Client, service_uri: &str, auth_token: &str, term: &str, index: u32, count: u32) -> Result<SmapiSearchResult, String> {
    let categories = "artists,albums,tracks,playlists";
    let body = format!(
        "<ns:search><ns:term>{term}</ns:term><ns:index>{index}</ns:index><ns:count>{count}</ns:count><ns:catalogId></ns:catalogId><ns:categories>{categories}</ns:categories></ns:search>"
    );
    let creds = make_credentials(auth_token);
    let resp = smapi_request(client, service_uri, "search", &body, Some(&creds)).await?;
    Ok(parse_search_response(&resp))
}

/// Get the root categories (like "my music", "playlists", "recommendations") for a service
pub async fn get_service_root(client: &Client, service_uri: &str, auth_token: &str) -> Result<SmapiBrowseResult, String> {
    // "root" is the standard SMAPI root container
    browse_music_service(client, service_uri, auth_token, "root", 0, 100).await
}

/// Search QQ Music via its public web API and return results with Sonos-playable URIs
pub async fn search_qq_music(client: &Client, term: &str, count: u32) -> Result<Vec<SmapiItem>, String> {
    let url = format!(
        "https://c.y.qq.com/soso/fcgi-bin/client_search_cp?w={}&format=json&p=1&n={}",
        urlencoding::encode(term),
        count
    );
    let resp = client
        .get(&url)
        .header("Referer", "https://y.qq.com")
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|e| format!("QQ Music search failed: {e}"))?;
    
    let text = resp.text().await.map_err(|e| format!("Read failed: {e}"))?;
    let json: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| format!("QQ Music JSON parse error: {e}"))?;
    
    let songs = json["data"]["song"]["list"]
        .as_array()
        .ok_or("No song list in QQ Music response")?;
    
    let mut items = Vec::new();
    for song in songs {
        let song_id = song["songid"].as_u64().unwrap_or(0);
        let title = song["songname"].as_str().unwrap_or("").to_string();
        let artist = song["singer"][0]["name"].as_str().unwrap_or("").to_string();
        let album = song["albumname"].as_str().unwrap_or("").to_string();
        let album_id = song["albumid"].as_u64().unwrap_or(0);
        
        if song_id == 0 || title.is_empty() { continue; }
        
        let uri = format!("x-sonos-http:SONG%3a{}%3aSQ.flac?sid=23&flags=8232&sn=7", song_id);
        let album_art = if album_id > 0 {
            format!("https://imgcache.qq.com/music/photo/album_300/{}/300_albumpic_{}_0.jpg", album_id % 100, album_id)
        } else { String::new() };
        
        items.push(SmapiItem {
            id: format!("SONG:{}", song_id),
            title,
            item_type: "track".to_string(),
            artist: Some(artist),
            album: Some(album),
            uri: Some(uri),
            album_art_uri: Some(album_art),
            description: None,
            playback_metadata: Some(format!(
                r#"<DIDL-Lite xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/" xmlns:r="urn:schemas-rinconnetworks-com:metadata-1-0/" xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/"><item id="10032020SONG%3a{}%3aSQ" parentID="00032020SONG%3a{}%3aSQ" restricted="true"><upnp:class>object.item.audioItem.musicTrack</upnp:class><desc id="cdudn" nameSpace="urn:schemas-rinconnetworks-com:metadata-1-0/">SA_RINCON5895_X_#Svc5895-0-Token</desc></item></DIDL-Lite>"#,
                song_id, song_id
            )),
        });
    }
    Ok(items)
}
