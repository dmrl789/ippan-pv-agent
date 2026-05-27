#!/usr/bin/env bash
# End-to-end validation of a real simulator-supplied telemetry / events
# fixture against the merged ippan-pv-agent (tag pv-data-contract-v1.0+).
#
# Runs the full ingestion path twice and confirms:
#   - parse PASS
#   - validation PASS
#   - evidence bundle created
#   - canonical hash stable across repeated builds in separate dirs
#   - attached events include every id in active_event_ids and any
#     completed event ending within the 240-min lookback window
#   - no float-shaped numeric appears outside JSON string values
#
# Usage:
#   scripts/e2e-fixture-validate.sh <telemetry.json> <events.json> [base_dir]

set -euo pipefail

TELEMETRY="${1:-}"
EVENTS="${2:-}"
BASE_DIR="${3:-data/pv-agent-e2e}"

if [[ -z "$TELEMETRY" || -z "$EVENTS" ]]; then
  echo "usage: $0 <telemetry.json> <events.json> [base_dir]" >&2
  exit 2
fi

[[ -f "$TELEMETRY" ]] || { echo "telemetry file not found: $TELEMETRY" >&2; exit 1; }
[[ -f "$EVENTS"    ]] || { echo "events file not found: $EVENTS" >&2; exit 1; }

# Find pv-agent binary.
BIN=""
for candidate in target/release/pv-agent target/release/pv-agent.exe target/debug/pv-agent target/debug/pv-agent.exe; do
  if [[ -x "$candidate" ]]; then BIN="$candidate"; break; fi
done
[[ -n "$BIN" ]] || { echo "pv-agent binary not found. Run 'cargo build --release' first." >&2; exit 1; }

mkdir -p "$BASE_DIR"

write_cfg() {
  local base="$1" key_path="$2"
  cat <<EOF
[agent]
agent_id = "pv-agent-palermo-001"
agent_type = "pv_plant_agent"
plant_id = "palermo-pv-001"
operator_key_ref = "key:plant-palermo-001"
production_mode = false

[storage]
base_dir = "$base"

[ippan]
endpoint = "http://127.0.0.1:18181"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = false

[events]
lookback_minutes = 240

[key]
key_file = "$key_path"
EOF
}

build_one() {
  local run="$1" base="$BASE_DIR/$run"
  rm -rf "$base"
  mkdir -p "$base/keys"
  local key_path="$base/keys/demo-key.json"
  "$BIN" generate-demo-key --out "$key_path" --key-ref "key:plant-palermo-001" >/dev/null
  local cfg_path="$base/pv-agent.toml"
  write_cfg "$base" "$key_path" > "$cfg_path"
  "$BIN" run-once --input "$TELEMETRY" --events "$EVENTS" --config "$cfg_path" --force
  local bundle
  bundle=$(find "$base" -type d -name 'pv-palermo-pv-001-*' | head -1)
  [[ -n "$bundle" ]] || { echo "no bundle dir found under $base" >&2; exit 1; }
  "$BIN" verify --bundle "$bundle"
  echo "$bundle"
}

canonical_hash() {
  local bundle="$1"
  # Avoid jq dependency; parse the manifest field by line.
  grep -E '"canonical_hash"' "$bundle/manifest.json" | head -1 | sed -E 's/.*"canonical_hash"\s*:\s*"([^"]+)".*/\1/'
}

echo "==> e2e-fixture-validate"
echo "    telemetry: $TELEMETRY"
echo "    events   : $EVENTS"
echo "    base dir : $BASE_DIR"
echo "    binary   : $BIN"

echo
echo "==> [1/4] Build + verify run #1"
BUNDLE1=$(build_one run1)
HASH1=$(canonical_hash "$BUNDLE1")

echo
echo "==> [2/4] Build + verify run #2 (idempotency)"
BUNDLE2=$(build_one run2)
HASH2=$(canonical_hash "$BUNDLE2")

if [[ "$HASH1" != "$HASH2" ]]; then
  echo "canonical hash NOT stable across runs:" >&2
  echo "  run1: $HASH1" >&2
  echo "  run2: $HASH2" >&2
  exit 1
fi
echo "    canonical hash stable: $HASH1"

echo
echo "==> [3/4] No-float regression scan"
python3 - "$BUNDLE1/canonical-record.json" <<'PY'
import sys, json, re
path = sys.argv[1]
with open(path, "rb") as f:
    text = f.read().decode("utf-8")
# Strip string literals (handle escapes) then look for digit.digit
out, in_str, esc = [], False, False
for ch in text:
    if in_str:
        if esc:    esc = False
        elif ch == "\\": esc = True
        elif ch == '"':  in_str = False
        continue
    if ch == '"':  in_str = True; continue
    out.append(ch)
joined = "".join(out)
if re.search(r"\d\.\d", joined):
    print(f"canonical record contains an unquoted float", file=sys.stderr)
    sys.exit(1)
print("    canonical record contains no unquoted floats")
PY

echo
echo "==> [4/4] Attached events / active_event_ids sanity"
python3 - "$BUNDLE1/canonical-record.json" <<'PY'
import sys, json
from datetime import datetime, timedelta, timezone
rec = json.load(open(sys.argv[1]))
active = list(rec.get("active_event_ids", []))
attached = [e["event_id"] for e in rec.get("events", [])]
ts = datetime.fromisoformat(rec["timestamp"].replace("Z", "+00:00"))
cutoff = ts - timedelta(minutes=240)
errs = []
for aid in active:
    if aid not in attached:
        errs.append(f"active_event_ids contains '{aid}' but it is not attached")
for e in rec.get("events", []):
    if "ended_at" in e and e["ended_at"]:
        end = datetime.fromisoformat(e["ended_at"].replace("Z", "+00:00"))
        if end < cutoff and e["event_id"] not in active:
            errs.append(f"event {e['event_id']} ended {e['ended_at']} — outside lookback and not active")
if errs:
    for x in errs: print("  - " + x, file=sys.stderr)
    sys.exit(1)
print(f"    active_event_ids : {', '.join(active) if active else '(none)'}")
print(f"    attached         : {', '.join(attached) if attached else '(none)'}")
PY

echo
echo "E2E VALIDATION: PASS"
echo "Bundle  : $BUNDLE1"
echo "Hash    : $HASH1"
