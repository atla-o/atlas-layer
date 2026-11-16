# Atlas Layer

Atlas Layer is a desktop overlay from Atla's Creation division. It keeps the machine productive, clean, and earning. It is one window with three independent parts. Each one has a switch in Settings, and each one can be turned off.

- **Office** is the pixel room. Each bot has a chair, a name plate, and a lamp: sage when idle, amber while working, cream when a reply is waiting. The whiteboard is the live to-do list. Click a bot and the chat stays inside the room. Office is on by default. When it is off, the window becomes a small status view instead of the room, and the desk watchers stay quiet.
- **Filter** checks desk text against a local word list for NSFW and harmful phrases. Nothing is uploaded. Turning it off stops scanning. It does not edit the hosts file or DNS.
- **Shared Compute** runs a capped local Roleplay sim tick when the machine is idle and plugged in. It is off until you turn it on, pauses on battery and while you are using the app, and counts credits on this computer. There is no paid payout network. The same `SpareEngine` interface is what another Creation sim can call.

Filter and Shared Compute run in this app. They still speak `ServiceRequest` / `ServiceReport` from `src-tauri/src/services/mod.rs`, so a later socket daemon can take over without changing Settings. `MockService` remains the contract fixture in tests. Shared Compute's `SpareEngine` is the reusable piece.

Nothing personal is baked in. Desks, paths, connector settings, and the three switches come from config. Secrets come from the environment or the system keyring, never from the config file, and they are not written to the log.

Atlas Layer is open source under the [MIT license](LICENSE).

## Why this stack

The window is [Tauri 2](https://v2.tauri.app/) on the system WebKitGTK. That is a native Wayland window on Fedora, including Asahi Remix, without a bundled browser. Rust owns the files, the HTTP calls, and the secrets. The page only draws.

The room is a 2D canvas, not Pixi or Phaser. It is one small bitmap, redrawn twice a second while the window is focused, and not at all while it is hidden or you have asked the system for reduced motion. A game loop next to a local model is the wrong cost for an app that sits open all day. Canvas 2D also avoids WebGL, which is the usual source of blank windows in WebKitGTK on Apple Silicon Linux.

Build it on the machine you run it on. The binary links the system WebKit and uses the system allocator, which is what behaves on Asahi's 16K pages. Do not copy an AppImage or a binary built on a 4K-page distro.

## Install on aarch64 Fedora (Asahi Remix 45)

System packages, once:

```bash
sudo dnf install -y \
  @development-tools \
  pkgconf-pkg-config \
  gtk3-devel \
  webkit2gtk4.1-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  openssl-devel \
  libsecret \
  curl wget file \
  nodejs npm
```

Rust from rustup (1.90 or newer; Fedora's packaged rustc is often too old for current Tauri):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

Node 20 or newer is required. If `node -v` is older than that, install a current Node before the next step.

Then clone and install, with no sudo:

```bash
git clone https://github.com/atla-o/atlas-layer
cd atlas-layer
./scripts/install.sh
atlas-layer
```

`install.sh` builds a release binary with the `custom-protocol` feature, so the window loads the files baked into the binary instead of the Vite dev server at `http://127.0.0.1:47331`. It copies that binary to `~/.local/bin/atlas-layer`, installs a `.desktop` file and icon under `~/.local`, and writes a starter config if you do not already have one.

Or search for “Atlas Layer” in the KDE menu. `~/.local/bin` needs to be on your `PATH` (it is, for a normal Fedora login).
