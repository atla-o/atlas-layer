#!/usr/bin/env bash
# Run the office against the sample board and a local mock of Ollama.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
mkdir -p examples/demo/inbox examples/demo/history
if [[ ! -e examples/demo/bots.json ]]; then
  ln -s ../../assets/defaults/bots.json examples/demo/bots.json
fi
if ! curl -sf -o /dev/null "http://127.0.0.1:${ATLA_MOCK_OLLAMA_PORT:-11439}/api/tags"; then
  node scripts/mock-ollama.mjs &
  echo $! > /tmp/atlas-layer-mock-ollama.pid
  sleep 0.3
fi
export ATLAS_LAYER_CONFIG="$root/examples/demo"
exec npm run tauri -- dev
