# Atlas Layer

Atlas Layer is a desktop overlay from Atla's Creation division. It keeps the machine productive, clean, and earning. It is one window with three independent parts. Each one has a switch in Settings, and each one can be turned off.

- **Office** is the pixel room. Each bot has a chair, a name plate, and a lamp: sage when idle, amber while working, cream when a reply is waiting. The whiteboard is the live to-do list. Click a bot and the chat stays inside the room. Office is on by default. When it is off, the window becomes a small status view instead of the room, and the desk watchers stay quiet.
- **Filter** is the harmful and NSFW content filter. The switch and the status line are here. The engine is not. Turning it on only arms a stub that reports it is standing by and scanning nothing.
- **Shared Compute** lends idle compute and earns. It is off by default. It only runs when the machine is idle or plugged in. The panel shows what is running, what has been contributed, and what has been earned, and it can be turned off in one click. The stub reports nothing running and zero earned.

Filter and Shared Compute are meant to move into a small background daemon. The app already talks to them through a JSON contract in `src-tauri/src/services/mod.rs` (`ServiceRequest` / `ServiceReport`, plus the `BackgroundService` trait). `MockService` answers in-process and starts no threads. A later socket client can speak the same messages without changing Settings.

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

If the window is blank on Asahi, try one of these and restart the app. They work around WebKit compositor bugs and cost more CPU, so leave them unset when the window is fine:

```bash
WEBKIT_DISABLE_COMPOSITING_MODE=1 atlas-layer
# or
WEBKIT_DISABLE_DMABUF_RENDERER=1 atlas-layer
```

Do not set `GDK_BACKEND=x11` unless Wayland itself is broken. The app is meant to be a native Wayland window.

## First run

Config lives in `$XDG_CONFIG_HOME/atlas-layer` (`~/.config/atlas-layer` unless you moved it):

- `config.json` — paths, connector settings, and the three switches under `features`. No secrets.
- `bots.json` — the desks. Add or remove objects in `bots`. Restart the app, or it will pick the file up the next time it reads config (a restart is the simple way).

The first launch copies `config.json` and `bots.json` from `~/.config/atla-office` when the new files are not there yet, so the Office, Filter, and Shared Compute switches carry over. The old directory is left in place. Chat history stays in `~/.local/share/atla-office/history` until a new history folder exists.

`ATLAS_LAYER_CONFIG` points at a different directory. `ATLA_OFFICE_CONFIG` still works if the new variable is unset. Paths inside the config may be absolute, `~/...`, or relative to that directory.

The switches, under `features`:

| Key | Default | What it does |
| --- | --- | --- |
| `office` | `true` | Shows the room. Off leaves a status window and pauses the desk watchers. |
| `filter` | `false` | Arms the filter stub. The engine is not installed, so nothing is scanned. |
| `sharedCompute` | `false` | Arms shared compute. It waits until the machine is idle or plugged in. The stub does not run work. |

A config written before these keys existed still loads. Missing switches use the defaults above, and saving a switch does not drop unknown fields.

The starter paths are:

| Key | Default |
| --- | --- |
| `paths.todo` | `~/Atla/AI/todo.json` |
| `paths.today` | `~/Atla/AI/atla-today.json` |
| `paths.personality` | `~/Atla/AI/personality.md` |
| `paths.inboxDir` | `~/Atla/AI/office-inbox` |
| `paths.inboxFile` | empty. Set this to tail a JSONL file as well as, or instead of, the directory. |
| `paths.historyDir` | empty, which means `$XDG_DATA_HOME/atlas-layer/history` (or the older `atla-office` history folder, if that is the only one) |

The inbox directory is created on launch so the cloud side has somewhere to drop replies. The todo file is created only when you add an item. A missing today cache is ignored. A today cache that exists but is not JSON is reported and left untouched.

`OLLAMA_HOST` overrides `ollama.baseUrl`. A value without a scheme gets `http://` in front, matching the Ollama convention.

To try the sample board and a fake local model without touching `~/Atla`:

```bash
./scripts/demo.sh
```

The sample list is `examples/todo.json`. The demo also merges `examples/today.json`.

## Desks

`bots.json`:

```json
{
  "bots": [
    {
      "id": "atlas",
      "name": "Atlas",
      "role": "Chief of staff",
      "connector": "relay-webhook",
      "accent": "#c45c3e",
      "hair": "#241c17",
      "accessory": "star"
    }
  ]
}
```

`id` is the stable key (letters, numbers, hyphens, underscores, up to 64 characters). It is what the relay sends in `bot`, and it is the history filename. `connector` is one of:

- `local-ollama` — a model on this machine. Optional `model` (default `qwen3:8b`) and `systemPrompt`.
- `relay-webhook` — Atlas and the other cloud desks. They all post to the same URL. The `bot` field is the router.
- `cursor-agents` — a Cursor cloud agent. Optional `agentId`. If it is empty, a unique name match from the API is used in memory. Pinning one from the chat writes `agentId` back and leaves every other field alone.
- `echo` — a local fake, for demos.

The seed list is Atlas, Products, Consumed, Creation, Covenant, Wellness, Modified, Devon, Local Atlas, Atlas site, and Roleplay. Colors and accessories are optional. Unknown connector names are skipped and reported.

## Whiteboard

The board shows `paths.todo`, then any items from `paths.today` whose id is not already on the todo list. Checking a today-only item copies it into `todo.json` and flips it there. The today file is never written.

Accepted todo shapes, both for the todo file and the today cache:

- a JSON array of items
- an object with an `items`, `todos`, `tasks`, or `list` array
- nested arrays of task objects, for a dashboard cache

An item is an object with a text field (`text`, `title`, `content`, `task`, `name`, or `label`) and an optional done field (`done`, `completed`, `checked`, or `complete`). A bare string is also an item. Unknown fields are kept. On save, the file is rewritten as two-space JSON with the original key order, so a sibling field like `"room"` stays put. If the file is not valid JSON, the app reports that and does not write.

## Chat

Click a desk. Messages are appended to `history/<id>.jsonl` as one JSON object per line:

```json
{"id":"m-…","role":"user","text":"…","ts":"2026-11-12T08:00:00Z","read":true}
```

`role` is `user`, `assistant`, or `system`. Reopening a desk shows that history. A lamp turns cream when an assistant line is still unread.

## Connectors

Each connector is a Rust `BotConnector` with an adapter behind it (`OllamaConnector`, `RelayConnector`, `CursorConnector`, `EchoConnector`). The desk's `connector` field picks the adapter.

### local-ollama

`POST {baseUrl}/api/chat` with `stream: true`, the model (`qwen3:8b` unless the desk or `ollama.model` says otherwise), and the recent history. If `paths.personality` exists it is sent as the system message, unless the desk sets `systemPrompt`. Tokens are the `message.content` fields. `message.thinking` is shown only when the model never produced content.

If Ollama cannot be reached and `ollama.echoIfUnavailable` is true, or `ATLAS_LAYER_ECHO=1` (or the older `ATLA_OFFICE_ECHO=1`), the desk answers in echo mode and says so. A model that is missing on a server that is up is reported as an error, not echoed.

### relay-webhook

Used for Atlas and every other cloud desk. One URL, one auth header. The desk id is the route.

`POST` JSON:

```json
{ "bot": "products", "text": "Sketch the shelf", "reply_to_id": "m-…" }
```

`reply_to_id` is the id of the user message just stored in history. Send it back as `in_reply_to`.

The URL is `relay.url`, unless `relay.urlEnv` (default `ATLAS_LAYER_WEBHOOK_URL`) is set. The Authorization value is `relay.authEnv` (default `ATLAS_LAYER_WEBHOOK_AUTH`) or, if that is empty, the keyring account `relay.authKeyringAccount` (default `webhook-auth`). A value with no space is sent as `Bearer <value>`. `Bearer …` and `Basic …` are sent as written. The header is omitted when no secret is set. A config carried over from `~/.config/atla-office` keeps the env var names it already had.

There is no field in the config for the token itself.

Replies are not read from the HTTP response. Drop them in the inbox.

Directory (`paths.inboxDir`), one JSON file per reply. The app moves a handled file to `processed/` and a file it cannot parse, or whose `bot` matches no desk, to `unmatched/`:

```json
{
  "bot": "atlas",
  "text": "Morning. The board is up.",
  "in_reply_to": "m-…",
  "ts": "2026-11-12T08:00:00Z"
}
```

`bot` may be the desk id or the display name. `in_reply_to` and `ts` are optional. Extra fields are ignored. `ts` should be ISO-8601.

Or a JSONL file (`paths.inboxFile`), one object per line, same shape. The app tails it and remembers the byte offset. A partial last line is left for the next write.

An example file is `examples/inbox-sample/atlas.json`. You can drop a copy in the inbox to see the lamp change.

### cursor-agents

[Cloud Agents API](https://cursor.com/docs/cloud-agent/api/endpoints) v1, with the v0 conversation endpoint for history (v1 has runs and a stream, not a transcript).

The key is `cursor.apiKeyEnv` (default `CURSOR_API_KEY`) or keyring account `cursor.apiKeyKeyringAccount` (default `cursor-api-key`). It is sent as `Authorization: Bearer <key>`, or as Basic if `cursor.auth` is `"basic"`. Store it with:

```bash
printf '%s' "$CURSOR_API_KEY" | secret-tool store \
  --label='Atlas Layer Cursor API key' \
  service atlas-layer \
  account cursor-api-key
```

The webhook token uses the same service and account `webhook-auth`. A key stored earlier under service `atla-office` is still read. Env vars win over the keyring, which is useful for a shell and for tests. `secret-tool` is in Fedora's `libsecret` package. If the lookup takes more than two seconds it is abandoned, so a missing daemon cannot stall the app.

Calls:

- `GET /v1/agents` — list, status, and name matching
- `POST /v1/agents/{id}/runs` with `{ "prompt": { "text": "…" } }` — follow-up
- `GET /v1/agents/{id}/runs/{runId}/stream` — assistant text as it arrives
- `GET /v0/agents/{id}/conversation` — history when you open the desk
- `GET /v1/agents/{id}` and `GET /v1/agents/{id}/runs/{latestRunId}` — fallback when v0 has no transcript

Status `ACTIVE` lights the lamp amber, unless a reply is already unread. The list is refreshed about once a minute (`cursor.pollSeconds`, minimum 15; `0` turns it off) and only if a key is present.

## Idle

The canvas timer is 500ms and stops when the window is hidden, blurred, or reduced motion is on. File watches use inotify. The Cursor poll is a single list call on a slow interval. Nothing busy-loops, and secrets are never logged. Errors that come back from a server are scrubbed if they echo a key.

## Develop

```bash
npm install
cargo test --manifest-path src-tauri/Cargo.toml
npm run tauri dev
```

The dev window loads Vite at <http://127.0.0.1:47331>. Opening that URL in a browser shows the same room with an in-memory echo, labeled as a preview. The desktop window is the one wired to the connectors.

`node scripts/gen-icon.mjs` writes the desktop icons and `src-tauri/icons/icon.png`. Tauri embeds that file at compile time. `cargo build` runs the script when the icon is not already there, so a fresh clone does not stop on a missing `icons/icon.png`.

`cargo test` covers the whiteboard round-trip (unknown fields kept, a broken file not overwritten), the relay body and Authorization header, Ollama streaming, the Cursor list / follow-up / conversation against a mock, and the inbox directory and JSONL tail.

## Layout

```
src/                  canvas office, chat, whiteboard
src-tauri/src/        config, history, inbox, connectors
assets/defaults/      seed config and desks
examples/             sample todo, today cache, personality, inbox file
scripts/install.sh    ~/.local install
packaging/            .desktop template and the O mark icon
src-tauri/icons/      window icon embedded by Tauri (`icon.png`)
```
