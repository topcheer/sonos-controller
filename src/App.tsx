import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useState, useEffect, useCallback, useRef } from "react";
import type {
  SonosDevice, ZoneGroup, TransportInfo, PositionInfo, MediaInfo,
  TrackInfo, QueueItem, BrowserItem, TransportSettings, SleepTimer,
  MusicService, SmapiItem, SmapiBrowseResult, SmapiSearchResult,
} from "./types";

function parseDidl(metadata: string, ip: string): TrackInfo | null {
  if (!metadata) return null;
  try {
    const u = metadata.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&").replace(/&quot;/g, '"');
    const doc = new DOMParser().parseFromString(u, "text/xml");
    const g = (t: string) => doc.querySelector(t)?.textContent?.trim() || "";
    let art = g("upnp\\:albumArtURI") || g("albumArtURI");
    if (art?.startsWith("/")) art = `http://${ip}:1400${art}`;
    return { title: g("dc\\:title") || g("title"), artist: g("dc\\:creator") || g("creator"), album: g("upnp\\:album") || g("album"), album_art_uri: art || "", duration: "", uri: g("res") };
  } catch { return null; }
}

function fmt(t: string): string {
  if (!t || t === "NOT_IMPLEMENTED") return "0:00";
  const p = t.split(":");
  if (p.length !== 3) return t;
  const h = +p[0], m = +p[1], s = +p[2];
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

function parseDur(d: string): number {
  if (!d || d === "NOT_IMPLEMENTED" || d === "0:00:00") return 0;
  const p = d.split(":");
  return p.length === 3 ? +p[0] * 3600 + +p[1] * 60 + +p[2] : 0;
}

type Tab = "queue" | "browse" | "services" | "groups" | "eq";

export default function App() {
  const [devices, setDevices] = useState<SonosDevice[]>([]);
  const [groups, setGroups] = useState<ZoneGroup[]>([]);
  const [selectedIp, setSelectedIp] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);
  const [transport, setTransport] = useState<TransportInfo | null>(null);
  const [position, setPosition] = useState<PositionInfo | null>(null);
  const [track, setTrack] = useState<TrackInfo | null>(null);
  const [volume, setVolume] = useState(0);
  const [muted, setMuted] = useState(false);
  const [duration, setDuration] = useState(0);
  const [curTime, setCurTime] = useState(0);
  const [settings, setSettings] = useState<TransportSettings | null>(null);
  const [sleepTimer, setSleepTimer] = useState<string>("");
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("queue");
  const [queue, setQueue] = useState<QueueItem[]>([]);
  const [browseItems, setBrowseItems] = useState<BrowserItem[]>([]);
  const [browsePath, setBrowsePath] = useState<{ id: string; title: string }[]>([{ id: "0", title: "Home" }]);
  const [bass, setBass] = useState(0);
  const [treble, setTreble] = useState(0);
  const [loudness, setLoudness] = useState(false);
  const [showTimer, setShowTimer] = useState(false);
  const [perVolume, setPerVolume] = useState<Record<string, number>>({});
  // Music Services
  const [musicServices, setMusicServices] = useState<MusicService[]>([]);
  const [serviceTokens, setServiceTokens] = useState<Record<number, string>>({});
  const [selectedService, setSelectedService] = useState<MusicService | null>(null);
  const [serviceBrowsePath, setServiceBrowsePath] = useState<{ id: string; title: string }[]>([]);
  const [serviceItems, setServiceItems] = useState<SmapiItem[]>([]);
  const [serviceSearchTerm, setServiceSearchTerm] = useState("");
  const [serviceSearchItems, setServiceSearchItems] = useState<SmapiItem[]>([]);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const progressRef = useRef<HTMLDivElement>(null);


  const scan = useCallback(async () => {
    setScanning(true); setError(null);
    try {
      let found = await invoke<SonosDevice[]>("discover_devices", { timeout: 8 });
      // Stable sort by room name
      found.sort((a, b) => (a.room_name || a.name).localeCompare(b.room_name || b.name));
      setDevices(found);
      if (found.length > 0) {
        let zgs = await invoke<ZoneGroup[]>("get_zone_groups", { ip: found[0].ip });
        // Stable sort groups by coordinator name
        zgs.sort((a, b) => (a.coordinator_name || a.id).localeCompare(b.coordinator_name || b.id));
        setGroups(zgs);
        if (!selectedIp) {
          const c = zgs.find(g => g.members.length > 0);
          setSelectedIp(c?.coordinator_ip || found[0].ip);
        }
      }
    } catch (e) { setError(String(e)); }
    finally { setScanning(false); }
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const poll = useCallback(async () => {
    if (!selectedIp) return;
    try {
      const [ti, pi] = await Promise.all([
        invoke<TransportInfo>("get_transport_info", { ip: selectedIp }),
        invoke<PositionInfo>("get_position_info", { ip: selectedIp }),
      ]);
      setTransport(ti);
      setPosition(pi);
      setDuration(parseDur(pi.track_duration)); setCurTime(parseDur(pi.rel_time));
      // Prefer TrackMetaData from PositionInfo, fallback to MediaInfo
      if (pi.track_metadata) {
        setTrack(parseDidl(pi.track_metadata, selectedIp));
      } else {
        try {
          const mi = await invoke<MediaInfo>("get_media_info", { ip: selectedIp });
          setTrack(mi.current_uri_metadata ? parseDidl(mi.current_uri_metadata, selectedIp) : null);
        } catch { setTrack(null); }
      }
      // Fire and forget volume/settings
      invoke<number>("get_volume", { ip: selectedIp }).then(v => setVolume(v)).catch(() => {});
      invoke<boolean>("get_mute", { ip: selectedIp }).then(m => setMuted(m)).catch(() => {});
      invoke<TransportSettings>("get_transport_settings", { ip: selectedIp }).then(s => setSettings(s)).catch(() => {});
      invoke<SleepTimer>("get_sleep_timer", { ip: selectedIp }).then(s => setSleepTimer(s.remaining_time)).catch(() => {});
    } catch {}
  }, [selectedIp]);

  useEffect(() => {
    if (!selectedIp) return;
    poll();
    pollRef.current = setInterval(poll, 3000);
    return () => { if (pollRef.current) clearInterval(pollRef.current); };
  }, [selectedIp, poll]);

  useEffect(() => { scan(); }, [scan]);

  const loadQueue = useCallback(async () => {
    if (!selectedIp) return;
    try {
      const items = await invoke<QueueItem[]>("get_queue", { ip: selectedIp, start: 0, limit: 500 });
      setQueue(items.map(q => ({ ...q, album_art_uri: q.album_art_uri?.startsWith("/") ? `http://${selectedIp}:1400${q.album_art_uri}` : q.album_art_uri })));
    } catch {}
  }, [selectedIp]);

  const loadBrowse = useCallback(async (objectId: string) => {
    if (!selectedIp) return;
    try {
      const items = await invoke<BrowserItem[]>("browse_directory", { ip: selectedIp, objectId, start: 0, limit: 500 });
      setBrowseItems(items.map(b => ({ ...b, album_art_uri: b.album_art_uri?.startsWith("/") ? `http://${selectedIp}:1400${b.album_art_uri}` : b.album_art_uri })));
    } catch {}
  }, [selectedIp]);

  const loadEq = useCallback(async () => {
    if (!selectedIp) return;
    try {
      const [b, t, l] = await Promise.all([
        invoke<number>("get_bass", { ip: selectedIp }),
        invoke<number>("get_treble", { ip: selectedIp }),
        invoke<boolean>("get_loudness", { ip: selectedIp }),
      ]);
      setBass(b); setTreble(t); setLoudness(l);
    } catch {}
  }, [selectedIp]);

  // Music Services
  const loadMusicServices = useCallback(async () => {
    if (!selectedIp) return;
    try {
      const services = await invoke<MusicService[]>("list_music_services", { ip: selectedIp });
      setMusicServices(services);
      // Load saved tokens from localStorage
      const saved = localStorage.getItem("sonos_service_tokens");
      if (saved) setServiceTokens(JSON.parse(saved));
    } catch {}
  }, [selectedIp]);

  const loadServiceRoot = useCallback(async (service: MusicService) => {
    const token = serviceTokens[service.id] || "";
    try {
      const result = await invoke<SmapiBrowseResult>("get_music_service_root", { serviceUri: service.secure_uri, authToken: token });
      setSelectedService(service);
      setServiceBrowsePath([{ id: "root", title: service.name }]);
      setServiceItems(result.items);
    } catch (e) {
      // For QQ Music and similar services where SMAPI browse fails, show search-only mode
      setSelectedService(service);
      setServiceBrowsePath([{ id: "root", title: service.name }]);
      setServiceItems([]);
      setError("");
    }
  }, [serviceTokens]);

  const browseServiceInto = useCallback(async (item: SmapiItem) => {
    if (!selectedService) return;
    const token = serviceTokens[selectedService.id] || "";
    if (item.item_type === "container") {
      try {
        const result = await invoke<SmapiBrowseResult>("browse_music_service", {
          serviceUri: selectedService.secure_uri, authToken: token, parentId: item.id, index: 0, count: 100,
        });
        setServiceBrowsePath([...serviceBrowsePath, { id: item.id, title: item.title }]);
        setServiceItems(result.items);
      } catch (e) { setError(String(e)); }
    } else if (item.uri && selectedIp) {
      try {
        await invoke("play_uri", { ip: selectedIp, uri: item.uri, metadata: item.playback_metadata || "" });
        setTimeout(poll, 500);
      } catch (e) { setError(String(e)); }
    }
  }, [selectedService, serviceTokens, serviceBrowsePath, selectedIp, poll]);

  const searchService = useCallback(async (term: string) => {
    if (!selectedService || !term.trim()) { setServiceSearchItems([]); return; }
    try {
      // For QQ Music (sid=23), use the direct web API
      if (selectedService.id === 23) {
        const results = await invoke<SmapiItem[]>("search_qq_music", { term });
        setServiceSearchItems(results);
        return;
      }
      const token = serviceTokens[selectedService.id] || "";
      const result = await invoke<SmapiSearchResult>("search_music_service", {
        serviceUri: selectedService.secure_uri, authToken: token, term, index: 0, count: 50,
      });
      setServiceSearchItems(result.items);
    } catch (e) { setError(String(e)); }
  }, [selectedService, serviceTokens]);

  useEffect(() => {
    if (tab === "queue") loadQueue();
    else if (tab === "browse") {
      loadBrowse(browsePath[browsePath.length - 1].id);
    } else if (tab === "eq") loadEq();
    else if (tab === "services") loadMusicServices();
  }, [tab, loadQueue, loadBrowse, loadEq, loadMusicServices, browsePath]);

  const cmd = async (c: string, args?: Record<string, unknown>) => {
    if (!selectedIp) return;
    try { await invoke(c, { ip: selectedIp, ...args }); setTimeout(poll, 500); }
    catch (e) { setError(String(e)); }
  };

  const playTrack = async (idx: number) => {
    if (!selectedIp) return;
    try { await invoke("play_from_queue", { ip: selectedIp, trackIndex: idx }); setTimeout(() => { poll(); loadQueue(); }, 800); }
    catch (e) { setError(String(e)); }
  };

  const browseInto = (item: BrowserItem) => {
    if (item.is_container) setBrowsePath([...browsePath, { id: item.object_id, title: item.title }]);
    else if (item.uri) invoke("play_uri", { ip: selectedIp!, uri: item.uri, metadata: "" }).then(() => setTimeout(poll, 500));
  };

  const cycleMode = () => {
    if (!settings) return;
    const modes = ["NORMAL", "REPEAT_ALL", "SHUFFLE_NOREPEAT", "SHUFFLE"];
    const i = modes.indexOf(settings.play_mode);
    cmd("set_play_mode", { mode: modes[(i + 1) % modes.length] });
  };

  const isPlaying = transport?.state === "PLAYING";
  const curGroup = groups.find(g => g.coordinator_ip === selectedIp || g.members.some(m => m.ip === selectedIp));
  const pct = duration > 0 ? (curTime / duration) * 100 : 0;

  useEffect(() => {
    if (tab === "groups" && curGroup) {
      (async () => {
        const vols: Record<string, number> = {};
        for (const m of curGroup.members) {
          try { vols[m.ip] = await invoke<number>("get_volume", { ip: m.ip }); } catch { vols[m.ip] = 0; }
        }
        setPerVolume(vols);
      })();
    }
  }, [tab, curGroup]);

  const modeLabel = () => {
    if (!settings) return { icon: "➡", label: "Normal" };
    switch (settings.play_mode) {
      case "REPEAT_ALL": return { icon: "🔁", label: "Repeat All" };
      case "SHUFFLE": return { icon: "🔀", label: "Shuffle+Repeat" };
      case "SHUFFLE_NOREPEAT": return { icon: "🔀", label: "Shuffle" };
      default: return { icon: "➡", label: "Normal" };
    }
  };

  return (
    <div className="flex h-screen bg-[#121212] text-white select-none overflow-hidden">
      {/* ===== SIDEBAR ===== */}
      <aside className="w-[280px] flex flex-col bg-[#000] flex-shrink-0">
        {/* Custom Title Bar - draggable */}
        <div data-tauri-drag-region className="flex items-center justify-between px-4 pt-3 pb-2">
          <div data-tauri-drag-region className="flex items-center gap-3">
            <div className="w-7 h-7 rounded-lg bg-gradient-to-br from-indigo-500 to-purple-600 flex items-center justify-center text-xs font-bold">S</div>
            <div data-tauri-drag-region>
              <div className="font-semibold text-[13px] tracking-tight">Sonos</div>
            </div>
          </div>
          <div className="flex items-center gap-1">
            <button onClick={() => getCurrentWindow().minimize()}
              className="w-7 h-7 rounded-md flex items-center justify-center hover:bg-white/10 transition-colors text-gray-400 hover:text-white">
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><path d="M5 12h14"/></svg>
            </button>
            <button onClick={() => getCurrentWindow().toggleMaximize()}
              className="w-7 h-7 rounded-md flex items-center justify-center hover:bg-white/10 transition-colors text-gray-400 hover:text-white">
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><rect x="5" y="5" width="14" height="14" rx="1"/></svg>
            </button>
            <button onClick={() => getCurrentWindow().close()}
              className="w-7 h-7 rounded-md flex items-center justify-center hover:bg-red-500/80 transition-colors text-gray-400 hover:text-white">
              <svg className="w-3.5 h-3.5" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><path d="M18 6L6 18M6 6l12 12"/></svg>
            </button>
          </div>
        </div>

        <div className="px-4 mb-2">
          <button onClick={scan} disabled={scanning}
            className="w-full py-2.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 active:scale-[0.98] disabled:opacity-50 text-sm font-medium transition-all flex items-center justify-center gap-2">
            {scanning ? <><span className="animate-spin">⟳</span> Searching...</> : <>
              <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><path d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"/></svg>
              Search Speakers
            </>}
          </button>
        </div>

        {/* Zone Groups */}
        <div className="px-4 mb-1"><div className="text-[10px] text-gray-600 font-semibold uppercase tracking-widest">Rooms</div></div>
        <nav className="flex-1 overflow-y-auto px-2 pb-4 space-y-0.5">
          {groups.map(g => {
            const active = selectedIp === g.coordinator_ip;
            return (
              <button key={g.id} onClick={() => setSelectedIp(g.coordinator_ip)}
                className={`w-full text-left px-3 py-2.5 rounded-xl transition-all duration-200 ${active
                  ? "bg-white/10 ring-1 ring-white/10"
                  : "hover:bg-white/[0.06] active:bg-white/[0.08]"}`}>
                <div className="flex items-center gap-3">
                  <div className={`w-8 h-8 rounded-full flex items-center justify-center text-sm ${active ? "bg-indigo-500" : "bg-white/[0.06]"}`}>
                    {g.members.length > 1 ? `${g.members.length}` : "♪"}
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className={`text-[13px] font-medium truncate ${active ? "text-white" : "text-gray-300"}`}>
                      {g.coordinator_name || g.members[0]?.room_name}
                    </div>
                    <div className="text-[11px] text-gray-600 truncate">
                      {g.members.map(m => m.room_name).join(" · ")}
                    </div>
                  </div>
                </div>
              </button>
            );
          })}
          {groups.length === 0 && !scanning && devices.map(d => (
            <button key={d.uid} onClick={() => setSelectedIp(d.ip)}
              className={`w-full text-left px-3 py-2.5 rounded-xl transition-all ${selectedIp === d.ip ? "bg-white/10" : "hover:bg-white/[0.06]"}`}>
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-white/[0.06] flex items-center justify-center text-sm">♪</div>
                <div><div className="text-[13px] font-medium">{d.room_name}</div><div className="text-[11px] text-gray-600">{d.model}</div></div>
              </div>
            </button>
          ))}
        </nav>
        {curGroup && (
          <div className="px-4 py-3 border-t border-white/[0.06] text-[11px] text-gray-600 text-center">
            {curGroup.members.length} speaker{curGroup.members.length > 1 ? "s" : ""} · {devices.length} total
          </div>
        )}
      </aside>

      {/* ===== MAIN ===== */}
      <main className="flex-1 flex flex-col min-w-0">
        {error && (
          <div className="mx-6 mt-4 bg-red-500/10 text-red-400 px-4 py-2.5 rounded-xl text-sm flex items-center justify-between border border-red-500/20">
            <span>{error}</span>
            <button onClick={() => setError(null)} className="ml-4 hover:text-red-300 text-lg leading-none">&times;</button>
          </div>
        )}

        {!selectedIp ? (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center space-y-4">
              <div className="w-24 h-24 rounded-3xl bg-white/[0.03] flex items-center justify-center mx-auto">
                <svg className="w-12 h-12 text-gray-800" fill="currentColor" viewBox="0 0 24 24"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02zM14 3.23v2.06c2.89.86 5 3.54 5 6.71s-2.11 5.85-5 6.71v2.06c4.01-.91 7-4.49 7-8.77s-2.99-7.86-7-8.77z"/></svg>
              </div>
              <div>
                <h2 className="text-xl font-semibold text-gray-400">Select a room</h2>
                <p className="text-sm text-gray-700 mt-1">Choose a speaker from the sidebar</p>
              </div>
            </div>
          </div>
        ) : (<>
          {/* ===== NOW PLAYING HERO ===== */}
          <div className="px-8 pt-8 pb-6">
            <div className="flex items-start gap-6">
              {/* Album Art */}
              <div className="w-[200px] h-[200px] rounded-2xl overflow-hidden flex-shrink-0 shadow-2xl shadow-black/50 bg-gradient-to-br from-indigo-900/30 to-purple-900/30">
                {track?.album_art_uri
                  ? <img src={track.album_art_uri} alt="" className="w-full h-full object-cover" />
                  : <div className="w-full h-full flex items-center justify-center">
                      <svg className="w-16 h-16 text-gray-800" fill="currentColor" viewBox="0 0 24 24"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
                    </div>}
              </div>

              {/* Track Info + Controls */}
              <div className="flex-1 flex flex-col justify-between py-2 min-w-0">
                <div>
                  <h1 className="text-2xl font-bold tracking-tight truncate">{track?.title || "Not Playing"}</h1>
                  <p className="text-base text-gray-400 mt-1 truncate">{track?.artist || "—"}</p>
                  <p className="text-sm text-gray-600 truncate">{track?.album || ""}</p>
                </div>

                {/* Transport Buttons */}
                <div className="flex items-center gap-4 mt-6">
                  <button onClick={cycleMode} className="w-9 h-9 rounded-full hover:bg-white/10 flex items-center justify-center transition-colors" title={modeLabel().label}>
                    <span className="text-sm">{modeLabel().icon}</span>
                  </button>

                  <button onClick={() => cmd("previous_track")} className="w-10 h-10 rounded-full hover:bg-white/10 flex items-center justify-center transition-colors">
                    <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24"><path d="M6 6h2v12H6zm3.5 6l8.5 6V6z"/></svg>
                  </button>

                  <button onClick={() => cmd(isPlaying ? "pause" : "play")}
                    className="w-14 h-14 rounded-full bg-white text-black hover:scale-105 active:scale-95 flex items-center justify-center transition-all shadow-lg shadow-white/10">
                    {isPlaying
                      ? <svg className="w-6 h-6" fill="currentColor" viewBox="0 0 24 24"><path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z"/></svg>
                      : <svg className="w-6 h-6 ml-1" fill="currentColor" viewBox="0 0 24 24"><path d="M8 5v14l11-7z"/></svg>}
                  </button>

                  <button onClick={() => cmd("next_track")} className="w-10 h-10 rounded-full hover:bg-white/10 flex items-center justify-center transition-colors">
                    <svg className="w-5 h-5" fill="currentColor" viewBox="0 0 24 24"><path d="M6 18l8.5-6L6 6v12zM16 6v12h2V6h-2z"/></svg>
                  </button>

                  <button onClick={() => cmd("set_crossfade", { on: !settings?.crossfade })}
                    className={`w-9 h-9 rounded-full flex items-center justify-center transition-colors ${settings?.crossfade ? "bg-indigo-500/20 text-indigo-400" : "hover:bg-white/10 text-gray-500"}`}
                    title="Crossfade">
                    <span className="text-sm">⇄</span>
                  </button>

                  {/* Sleep Timer */}
                  <div className="relative ml-2">
                    <button onClick={() => setShowTimer(!showTimer)}
                      className={`w-9 h-9 rounded-full flex items-center justify-center transition-colors ${sleepTimer && sleepTimer !== "0:00:00" ? "bg-amber-500/20 text-amber-400" : "hover:bg-white/10 text-gray-500"}`}
                      title="Sleep Timer">
                      <svg className="w-4 h-4" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
                    </button>
                    {showTimer && (
                      <div className="absolute left-0 top-12 bg-[#282828] rounded-xl p-2 shadow-2xl border border-white/10 z-50 min-w-[140px]">
                        <div className="text-[10px] text-gray-500 uppercase tracking-wider px-3 py-1.5 font-semibold">Sleep Timer</div>
                        {[15, 30, 60, 90, 120].map(m => (
                          <button key={m} onClick={() => { cmd("set_sleep_timer", { seconds: m * 60 }); setShowTimer(false); }}
                            className="block w-full text-left px-3 py-2 text-sm rounded-lg hover:bg-white/10 transition-colors">{m} min</button>
                        ))}
                        <div className="border-t border-white/10 my-1" />
                        <button onClick={() => { cmd("set_sleep_timer", { seconds: 0 }); setShowTimer(false); }}
                          className="block w-full text-left px-3 py-2 text-sm rounded-lg hover:bg-white/10 transition-colors text-red-400">Turn Off</button>
                      </div>
                    )}
                  </div>
                </div>
              </div>
            </div>

            {/* Progress Bar */}
            <div className="mt-6 flex items-center gap-3">
              <span className="text-[11px] text-gray-500 w-12 text-right font-mono">{fmt(position?.rel_time || "")}</span>
              <div ref={progressRef} className="flex-1 h-1.5 bg-white/[0.08] rounded-full cursor-pointer group relative"
                onClick={e => {
                  const r = e.currentTarget.getBoundingClientRect();
                  const sec = Math.floor(((e.clientX - r.left) / r.width) * duration);
                  const h = Math.floor(sec / 3600), m = Math.floor((sec % 3600) / 60), s = sec % 60;
                  cmd("seek", { target: `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` });
                }}>
                <div className="h-full bg-white group-hover:bg-indigo-400 rounded-full transition-colors relative" style={{ width: `${pct}%` }}>
                  <div className="absolute right-0 top-1/2 -translate-y-1/2 w-3 h-3 bg-white rounded-full opacity-0 group-hover:opacity-100 transition-opacity shadow" />
                </div>
              </div>
              <span className="text-[11px] text-gray-500 w-12 font-mono">{fmt(position?.track_duration || "")}</span>
            </div>

            {/* Volume Row */}
            <div className="mt-3 flex items-center justify-end gap-2">
              <button onClick={() => cmd("set_mute", { mute: !muted })} className="p-1.5 rounded-lg hover:bg-white/10 transition-colors">
                {muted
                  ? <svg className="w-4 h-4 text-red-400" fill="currentColor" viewBox="0 0 24 24"><path d="M16.5 12c0-1.77-1.02-3.29-2.5-4.03v2.21l2.45 2.45c.03-.2.05-.41.05-.63zm2.5 0c0 .94-.2 1.82-.54 2.64l1.51 1.51A8.796 8.796 0 0021 12c0-4.28-2.99-7.86-7-8.77v2.06c2.89.86 5 3.54 5 6.71zM4.27 3L3 4.27 7.73 9H3v6h4l5 5v-6.73l4.25 4.25c-.67.52-1.42.93-2.25 1.18v2.06a8.99 8.99 0 003.69-1.81L19.73 21 21 19.73l-9-9L4.27 3zM12 4L9.91 6.09 12 8.18V4z"/></svg>
                  : <svg className="w-4 h-4 text-gray-400" fill="currentColor" viewBox="0 0 24 24"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02z"/></svg>}
              </button>
              <svg className="w-4 h-4 text-gray-600" fill="currentColor" viewBox="0 0 24 24"><path d="M3 9v6h4l5 5V4L7 9H3z"/></svg>
              <input type="range" min="0" max="100" value={volume}
                onChange={e => { const v = +e.target.value; setVolume(v); cmd("set_volume", { volume: v }); }}
                className="w-36 h-1 bg-white/10 rounded-full appearance-none cursor-pointer
                  [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-3 [&::-webkit-slider-thumb]:h-3 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:shadow-md" />
              <span className="text-[11px] text-gray-600 w-6">{volume}</span>
            </div>
          </div>

          {/* ===== TABS ===== */}
          <div className="px-8">
            <div className="flex gap-1 bg-white/[0.03] rounded-xl p-1">
              {([["queue", "Queue"], ["browse", "Browse"], ["services", "Services"], ["groups", "Groups"], ["eq", "EQ"]] as [Tab, string][]).map(([id, label]) => (
                <button key={id} onClick={() => setTab(id)}
                  className={`flex-1 py-2 rounded-lg text-sm font-medium transition-all ${tab === id
                    ? "bg-white/10 text-white shadow-sm"
                    : "text-gray-500 hover:text-gray-300"}`}>
                  {label}
                </button>
              ))}
            </div>
          </div>

          {/* ===== TAB CONTENT ===== */}
          <div className="flex-1 overflow-y-auto px-8 pt-4 pb-8">

            {/* Queue */}
            {tab === "queue" && (
              <div className="space-y-0.5">
                {queue.map((item, i) => {
                  const active = position?.track === i + 1;
                  return (
                    <button key={i} onClick={() => playTrack(i)}
                      className={`w-full flex items-center gap-4 px-4 py-3 rounded-xl transition-all text-left group ${active
                        ? "bg-indigo-500/10"
                        : "hover:bg-white/[0.04]"}`}>
                      <div className="w-5 text-center flex-shrink-0">
                        {active && isPlaying
                          ? <span className="text-indigo-400"><svg className="w-4 h-4 animate-pulse" fill="currentColor" viewBox="0 0 24 24"><path d="M3 9v6h4l5 5V4L7 9H3zm13.5 3c0-1.77-1.02-3.29-2.5-4.03v8.05c1.48-.73 2.5-2.25 2.5-4.02z"/></svg></span>
                          : <span className={`text-[11px] ${active ? "text-indigo-400" : "text-gray-700 group-hover:text-gray-400"}`}>{i + 1}</span>}
                      </div>
                      {item.album_art_uri
                        ? <img src={item.album_art_uri} className="w-10 h-10 rounded-lg object-cover flex-shrink-0 shadow" />
                        : <div className="w-10 h-10 rounded-lg bg-white/[0.04] flex items-center justify-center flex-shrink-0">
                            <svg className="w-4 h-4 text-gray-700" fill="currentColor" viewBox="0 0 24 24"><path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/></svg>
                          </div>}
                      <div className="flex-1 min-w-0">
                        <div className={`text-sm truncate ${active ? "text-indigo-300 font-medium" : ""}`}>{item.title}</div>
                        <div className="text-[11px] text-gray-600 truncate">{item.artist}{item.album ? ` · ${item.album}` : ""}</div>
                      </div>
                    </button>
                  );
                })}
                {queue.length === 0 && <div className="py-16 text-center text-gray-600">Queue is empty</div>}
              </div>
            )}

            {/* Browse */}
            {tab === "browse" && (<>
              <div className="flex items-center gap-1 text-sm text-gray-600 mb-3 flex-wrap">
                {browsePath.map((p, i) => (
                  <span key={i} className="flex items-center">
                    {i > 0 && <svg className="w-3 h-3 mx-1 text-gray-700" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><path d="M9 5l7 7-7 7"/></svg>}
                    <button onClick={() => setBrowsePath(browsePath.slice(0, i + 1))}
                      className={`hover:text-white transition-colors rounded px-1.5 py-0.5 ${i === browsePath.length - 1 ? "text-white bg-white/10" : ""}`}>{p.title}</button>
                  </span>
                ))}
              </div>
              <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
                {browseItems.map((item, i) => {
                  const icon = item.object_id.startsWith("A:") ? "🎵" : item.object_id.startsWith("S:") ? "💾" : item.object_id.startsWith("SQ:") ? "📋" : item.object_id.startsWith("R:") ? "📻" : item.object_id.startsWith("FV:") ? "⭐" : item.object_id.startsWith("Q:") ? "🎶" : item.is_container ? "📁" : "🎵";
                  return (
                    <button key={i} onClick={() => browseInto(item)}
                      className="bg-white/[0.03] hover:bg-white/[0.06] rounded-xl p-3 transition-all text-left group">
                      <div className="aspect-square rounded-lg bg-white/[0.04] flex items-center justify-center mb-3 overflow-hidden">
                        {item.album_art_uri
                          ? <img src={item.album_art_uri} className="w-full h-full object-cover" />
                          : <span className="text-3xl opacity-30">{icon}</span>}
                      </div>
                      <div className="text-sm font-medium truncate">{item.title}</div>
                      <div className="text-[11px] text-gray-600">{item.is_container ? (item.object_id === "0" ? "" : "Folder") : "Track"}</div>
                    </button>
                  );
                })}
              </div>
              {browseItems.length === 0 && <div className="py-16 text-center text-gray-600">Empty</div>}
            </>)}

            {/* Services */}
            {tab === "services" && (
              <div className="space-y-4">
                {!selectedService ? (<>
                  <div className="text-xs text-gray-600 mb-2">
                    Only services with linked accounts are shown. Tap to browse or login.
                  </div>
                  <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
                    {musicServices.map(s => (
                        <button key={s.id}
                          onClick={() => loadServiceRoot(s)}
                          className="bg-white/[0.03] hover:bg-white/[0.06] rounded-xl p-4 transition-all text-left border border-white/[0.06] hover:border-indigo-500/30 group">
                          <div className="flex items-center gap-3">
                            <div className="w-10 h-10 rounded-xl flex items-center justify-center text-lg bg-indigo-500/20">
                              {s.name.includes("QQ") ? "Q" : s.name.includes("网易") ? "☁" : s.name.includes("酷我") ? "K" : s.name.includes("Apple") ? "A" : s.name.includes("Spotify") ? "S" : s.name.includes("TuneIn") ? "📻" : s.name.includes("Tidal") ? "T" : "🎵"}
                            </div>
                            <div className="flex-1 min-w-0">
                              <div className="text-sm font-medium truncate">{s.name}</div>
                              <div className="text-[11px] text-gray-600">Tap to browse</div>
                            </div>
                          </div>
                        </button>
                    ))}
                    {musicServices.length === 0 && (
                      <div className="col-span-full py-16 text-center text-gray-600">No configured services found</div>
                    )}
                  </div>
                </>) : (<>
                  {/* Service browse view */}
                  <div className="flex items-center justify-between mb-2">
                    <div className="flex items-center gap-1 text-sm text-gray-600 flex-wrap">
                      <button onClick={() => { setSelectedService(null); setServiceItems([]); setServiceSearchItems([]); setServiceSearchTerm(""); }}
                        className="hover:text-white transition-colors rounded px-1.5 py-0.5">{selectedService.name}</button>
                      {serviceBrowsePath.map((p, i) => (
                        <span key={i} className="flex items-center">
                          <svg className="w-3 h-3 mx-1 text-gray-700" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><path d="M9 5l7 7-7 7"/></svg>
                          <button onClick={async () => {
                            setServiceBrowsePath(serviceBrowsePath.slice(0, i + 1));
                            try {
                              const token = serviceTokens[selectedService.id] || "";
                              const result = await invoke<SmapiBrowseResult>("browse_music_service", {
                                serviceUri: selectedService.secure_uri, authToken: token || "", parentId: p.id, index: 0, count: 100,
                              });
                              setServiceItems(result.items);
                            } catch {}
                          }}
                            className={`hover:text-white transition-colors rounded px-1.5 py-0.5 ${i === serviceBrowsePath.length - 1 ? "text-white bg-white/10" : ""}`}>{p.title}</button>
                        </span>
                      ))}
                    </div>
                    <button onClick={() => { setSelectedService(null); setServiceItems([]); setServiceSearchItems([]); setServiceSearchTerm(""); }}
                      className="text-xs text-gray-500 hover:text-white px-2 py-1 rounded-lg hover:bg-white/[0.06] transition-colors">Back</button>
                  </div>

                  {/* Search bar */}
                  <div className="relative mb-3">
                    <input type="text" placeholder={`Search ${selectedService.name}...`}
                      value={serviceSearchTerm}
                      onChange={e => { setServiceSearchTerm(e.target.value); if (e.target.value.trim()) searchService(e.target.value); else setServiceSearchItems([]); }}
                      className="w-full bg-white/[0.04] border border-white/[0.06] rounded-xl px-4 py-2.5 text-sm placeholder:text-gray-600 focus:outline-none focus:border-indigo-500/50" />
                    {serviceSearchTerm && (
                      <button onClick={() => { setServiceSearchTerm(""); setServiceSearchItems([]); }}
                        className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-600 hover:text-white">&times;</button>
                    )}
                  </div>

                  {/* Items */}
                  <div className="space-y-0.5">
                    {(serviceSearchTerm ? serviceSearchItems : serviceItems).map((item, i) => (
                      <button key={i} onClick={() => browseServiceInto(item)}
                        className="w-full flex items-center gap-4 px-4 py-3 rounded-xl transition-all text-left group hover:bg-white/[0.04]">
                        <div className="w-10 h-10 rounded-lg bg-white/[0.04] flex items-center justify-center flex-shrink-0 overflow-hidden">
                          {item.album_art_uri
                            ? <img src={item.album_art_uri} className="w-full h-full object-cover" />
                            : <span className="text-lg opacity-30">{item.item_type === "container" ? "📁" : "🎵"}</span>}
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="text-sm truncate">{item.title}</div>
                          <div className="text-[11px] text-gray-600 truncate">
                            {item.artist || ""}{item.album ? ` · ${item.album}` : ""}
                          </div>
                        </div>
                        {item.item_type === "container" && (
                          <svg className="w-4 h-4 text-gray-700" fill="none" stroke="currentColor" strokeWidth={2} viewBox="0 0 24 24"><path d="M9 5l7 7-7 7"/></svg>
                        )}
                      </button>
                    ))}
                    {(serviceSearchTerm ? serviceSearchItems : serviceItems).length === 0 && (
                      <div className="py-16 text-center text-gray-600">
                        {serviceSearchTerm ? "No results" : 
                         selectedService?.id === 23 ? "Search to find QQ Music songs" : "Empty"}
                      </div>
                    )}
                  </div>
                </>)}
              </div>
            )}

            {/* Groups */}
            {tab === "groups" && (
              <div className="space-y-4">
                {curGroup && (
                  <div className="bg-white/[0.03] rounded-2xl p-5 border border-white/[0.06]">
                    <div className="flex items-center justify-between mb-4">
                      <div>
                        <h3 className="text-base font-semibold">{curGroup.coordinator_name || "Group"}</h3>
                        <p className="text-xs text-gray-600 mt-0.5">{curGroup.members.length} speaker{curGroup.members.length > 1 ? "s" : ""} grouped</p>
                      </div>
                      <button onClick={() => curGroup.members.forEach(m => { if (m.ip !== selectedIp) invoke("leave_group", { ip: m.ip }); })}
                        className="text-xs text-red-400/70 hover:text-red-400 px-3 py-1.5 rounded-lg hover:bg-red-500/10 transition-colors">Ungroup</button>
                    </div>
                    <div className="space-y-3">
                      {curGroup.members.map(m => (
                        <div key={m.uid} className="flex items-center gap-4 py-2">
                          <div className={`w-8 h-8 rounded-full flex items-center justify-center text-xs ${m.is_coordinator ? "bg-amber-500/20 text-amber-400" : "bg-white/[0.06] text-gray-500"}`}>
                            {m.is_coordinator ? "★" : "♪"}
                          </div>
                          <div className="flex-1 min-w-0">
                            <div className="text-sm font-medium">{m.room_name}</div>
                            <div className="text-[10px] text-gray-700">{m.ip}{m.is_coordinator ? " · Coordinator" : ""}</div>
                          </div>
                          <div className="flex items-center gap-2 w-36">
                            <svg className="w-3 h-3 text-gray-600" fill="currentColor" viewBox="0 0 24 24"><path d="M3 9v6h4l5 5V4L7 9H3z"/></svg>
                            <input type="range" min="0" max="100" value={perVolume[m.ip] ?? 50}
                              onChange={e => { const v = +e.target.value; setPerVolume({ ...perVolume, [m.ip]: v }); invoke("set_volume", { ip: m.ip, volume: v }); }}
                              className="flex-1 h-1 bg-white/10 rounded-full appearance-none cursor-pointer [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-2.5 [&::-webkit-slider-thumb]:h-2.5 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white" />
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                )}
                {(devices.filter(d => !curGroup?.members.some(m => m.ip === d.ip) && !curGroup?.hidden_ips.includes(d.ip)).length > 0) && (
                  <div>
                    <div className="text-xs text-gray-600 font-semibold uppercase tracking-wider mb-3">Add to Group</div>
                    <div className="grid grid-cols-2 gap-3">
                      {devices.filter(d => !curGroup?.members.some(m => m.ip === d.ip) && !curGroup?.hidden_ips.includes(d.ip)).map(d => (
                        <button key={d.uid} onClick={() => cmd("join_group", { coordinatorIp: selectedIp })}
                          className="bg-white/[0.03] hover:bg-white/[0.06] rounded-xl p-4 transition-all text-left border border-white/[0.06] hover:border-indigo-500/30">
                          <div className="flex items-center gap-3">
                            <div className="w-10 h-10 rounded-full bg-white/[0.06] flex items-center justify-center text-sm">♪</div>
                            <div>
                              <div className="text-sm font-medium">{d.room_name}</div>
                              <div className="text-[11px] text-gray-600">{d.model}</div>
                            </div>
                          </div>
                        </button>
                      ))}
                    </div>
                  </div>
                )}
              </div>
            )}

            {/* EQ */}
            {tab === "eq" && (
              <div className="max-w-lg space-y-6">
                <div className="bg-white/[0.03] rounded-2xl p-5 border border-white/[0.06] space-y-5">
                  <h3 className="text-sm font-semibold text-gray-300">Tone Controls</h3>
                  {([["Bass", bass, setBass, "set_bass"], ["Treble", treble, setTreble, "set_treble"]] as const).map(([label, val, set, fn]) => (
                    <div key={label}>
                      <div className="flex items-center justify-between mb-2">
                        <span className="text-sm text-gray-400">{label}</span>
                        <span className={`text-sm font-mono w-8 text-center ${val > 0 ? "text-indigo-400" : val < 0 ? "text-orange-400" : "text-gray-600"}`}>
                          {val > 0 ? `+${val}` : val}
                        </span>
                      </div>
                      <input type="range" min="-10" max="10" value={val}
                        onChange={e => { const v = +e.target.value; set(v); cmd(fn, { value: v }); }}
                        className="w-full h-1.5 bg-white/[0.08] rounded-full appearance-none cursor-pointer
                          [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-4 [&::-webkit-slider-thumb]:h-4 [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white [&::-webkit-slider-thumb]:shadow-md" />
                      <div className="flex justify-between text-[10px] text-gray-700 mt-1"><span>-10</span><span>0</span><span>+10</span></div>
                    </div>
                  ))}
                  <div className="flex items-center justify-between pt-2 border-t border-white/[0.06]">
                    <span className="text-sm text-gray-400">Loudness</span>
                    <button onClick={() => { const v = !loudness; setLoudness(v); cmd("set_loudness", { on: v }); }}
                      className={`w-11 h-6 rounded-full transition-colors duration-200 relative ${loudness ? "bg-indigo-500" : "bg-white/10"}`}>
                      <div className={`w-5 h-5 rounded-full bg-white shadow absolute top-0.5 transition-all duration-200 ${loudness ? "left-[22px]" : "left-0.5"}`} />
                    </button>
                  </div>
                </div>

                <div className="bg-white/[0.03] rounded-2xl p-5 border border-white/[0.06] space-y-4">
                  <h3 className="text-sm font-semibold text-gray-300">Play Mode</h3>
                  <div className="grid grid-cols-2 gap-2">
                    {(["NORMAL", "REPEAT_ALL", "SHUFFLE_NOREPEAT", "SHUFFLE"] as const).map(mode => (
                      <button key={mode} onClick={() => cmd("set_play_mode", { mode })}
                        className={`py-2.5 px-3 rounded-xl text-xs font-medium transition-all ${settings?.play_mode === mode
                          ? "bg-indigo-500/20 text-indigo-300 ring-1 ring-indigo-500/30"
                          : "bg-white/[0.04] text-gray-500 hover:bg-white/[0.08]"}`}>
                        {mode.replace(/_/g, " ")}
                      </button>
                    ))}
                  </div>
                  <div className="flex items-center justify-between pt-3 border-t border-white/[0.06]">
                    <span className="text-sm text-gray-400">Crossfade</span>
                    <button onClick={() => cmd("set_crossfade", { on: !settings?.crossfade })}
                      className={`w-11 h-6 rounded-full transition-colors duration-200 relative ${settings?.crossfade ? "bg-indigo-500" : "bg-white/10"}`}>
                      <div className={`w-5 h-5 rounded-full bg-white shadow absolute top-0.5 transition-all duration-200 ${settings?.crossfade ? "left-[22px]" : "left-0.5"}`} />
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>
        </>)}
      </main>
    </div>
  );
}
