# Sonos Controller for Linux

An unofficial Sonos controller built with **Rust + Tauri + React**. Designed as a lightweight, native-feeling replacement for the abandoned Electron-based Sonos controller on Linux desktops.

![Screenshot](docs/screenshot.png)

## Features

- **Multi-room control** — Browse and select speakers/groups from the sidebar
- **Now Playing** — Full playback controls (play/pause, next/prev, seek, volume)
- **Queue management** — View, reorder, and clear the play queue
- **Music library browsing** — Browse local music shares via UPnP ContentDirectory
- **Music services** — Browse and play from online music services:
  - **QQ Music** — Search songs via web API, play directly on Sonos (no login required)
  - **TuneIn** — Browse and play radio stations (anonymous access)
  - **Other services** — Browse via SMAPI where supported
- **Group management** — Join/leave speaker groups
- **EQ controls** — Bass, treble, and loudness adjustment per speaker
- **Sleep timer** — Set per-speaker sleep timers
- **Custom title bar** — Seamless dark UI with no system chrome
- **Low resource usage** — ~30MB RAM (vs 300MB+ for Electron)

## Supported Architectures

| Format | Architecture | Notes |
|--------|-------------|-------|
| AppImage | x86_64 | Portable, no install needed |
| .deb | x86_64 | Ubuntu / Debian |
| .rpm | x86_64 | Fedora / openSUSE |

## Download

Check the [Releases](https://github.com/topcheer/sonos-controller/releases) page for pre-built packages.

## Build from Source

### Prerequisites

- Rust 1.70+ (`rustup`)
- Node.js 18+ and npm
- System dependencies (Ubuntu/Debian):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```
- System dependencies (Fedora):
  ```bash
  sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libxdo-devel libappindicator-gtk3-devel librsvg2-devel gcc
  ```

### Build

```bash
git clone https://github.com/topcheer/sonos-controller.git
cd sonos-controller
npm install
npm run tauri build
```

Output binaries are in `src-tauri/target/release/bundle/`.

### Development

```bash
npm run tauri dev
```

## How It Works

The controller communicates with Sonos speakers over the local network using UPnP/SOAP:

- **Device discovery** — SSDP (UPnP) to find speakers on the LAN
- **Playback control** — AVTransport UPnP service
- **Queue management** — AVTransport + ContentDirectory
- **Music services** — SMAPI (Sonos Music API) + service-specific web APIs
- **QQ Music playback** — Songs found via QQ Music search API are played using `x-sonos-http` URIs with speaker-stored credentials (`SA_RINCON` token in DIDL metadata)

## Configuration

No configuration needed — the controller auto-discovers Sonos speakers on your network.

Music service tokens (if applicable) are stored in browser localStorage.

## Compatibility

- **Sonos S1** and **S2** speakers
- **Linux** (X11 / Wayland)
- Tested with: Sonos One, Sonos Play:1, Sonos Play:5, Sonos Beam

## Limitations

- Online music service browsing depends on third-party SMAPI endpoints (some may be unreliable)
- QQ Music OAuth login page is currently broken on Sonos's side — search-and-play works as a workaround
- No Sonos Cloud API access (local network only)
- No alarm management yet

## License

MIT
