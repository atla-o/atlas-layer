#!/usr/bin/env bash
# Install Atlas Layer into the current user's ~/.local. No sudo.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

if ! command -v npm >/dev/null || ! command -v cargo >/dev/null; then
  echo "Need Node.js 20+ and Rust (rustup) on PATH." >&2
  exit 1
fi

node scripts/gen-icon.mjs
npm install
npm run build
# custom-protocol embeds the built page. Without it the binary loads devUrl
# (http://127.0.0.1:47331) and the installed app shows connection refused.
cargo build --release --manifest-path src-tauri/Cargo.toml --features custom-protocol

bindir="${HOME}/.local/bin"
appdir="${HOME}/.local/share/applications"
icondir="${HOME}/.local/share/icons/hicolor"
cfg="${XDG_CONFIG_HOME:-$HOME/.config}/atlas-layer"
legacy="${XDG_CONFIG_HOME:-$HOME/.config}/atla-office"
mkdir -p "$bindir" "$appdir" "$icondir/128x128/apps" "$icondir/48x48/apps" "$cfg"

# `install` is a shell function on some machines and would swallow -m.
cp src-tauri/target/release/atlas-layer "$bindir/atlas-layer"
chmod 755 "$bindir/atlas-layer"
cp packaging/icon-128.png "$icondir/128x128/apps/atlas-layer.png"
chmod 644 "$icondir/128x128/apps/atlas-layer.png"
cp packaging/icon-48.png "$icondir/48x48/apps/atlas-layer.png"
chmod 644 "$icondir/48x48/apps/atlas-layer.png"

sed \
  -e "s|@BIN@|$bindir/atlas-layer|" \
  -e "s|@ICON@|$icondir/128x128/apps/atlas-layer.png|" \
  packaging/atlas-layer.desktop.in > "$appdir/atlas-layer.desktop"
chmod 644 "$appdir/atlas-layer.desktop"

# Drop the previous launcher name so the menu shows one app.
rm -f "$bindir/atla-office" "$appdir/atla-office.desktop"

if [[ ! -f "$cfg/config.json" && -f "$legacy/config.json" ]]; then
  cp "$legacy/config.json" "$cfg/config.json"
  if [[ ! -f "$cfg/bots.json" && -f "$legacy/bots.json" ]]; then
    cp "$legacy/bots.json" "$cfg/bots.json"
  fi
  echo "Carried settings over from $legacy"
fi
if [[ ! -f "$cfg/config.json" ]]; then
  cp assets/defaults/config.json "$cfg/config.json"
fi
if [[ ! -f "$cfg/bots.json" ]]; then
  cp assets/defaults/bots.json "$cfg/bots.json"
fi

if command -v update-desktop-database >/dev/null; then
  update-desktop-database "$appdir" || true
fi
if command -v gtk-update-icon-cache >/dev/null; then
  gtk-update-icon-cache -f "$icondir" || true
fi

echo "Installed. Launch with: atlas-layer"
echo "Config: $cfg"
