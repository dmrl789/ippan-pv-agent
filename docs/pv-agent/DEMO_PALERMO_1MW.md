# Palermo 1MW demo

This walkthrough produces a complete evidence bundle from canned Palermo
1MW data, verifies it, and (optionally) anchors the commitment to a
local IPPANCENT staging endpoint.

## 1. Build

```bash
cargo build --release
```

The binary is at `target/release/pv-agent`.

## 2. Run the demo

```bash
./target/release/pv-agent demo --plant palermo-1mw
```

Expected output:

```
PV Agent Demo — Palermo 1MW

Telemetry interval: 15 minutes
GHI: 554 W/m²
Temperature: 20.5 °C
DC power: 492.5 kW
AC power: 471.6 kW
Meter power: 463.1 kW
Performance ratio: 0.859
Energy since start: 463.1 kWh

Canonical record created: YES
Signature created: YES
Evidence bundle saved: YES
IPPAN L1 anchor submitted: NO
Reason: submit_anchors=false

Bundle:
data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

The demo command:

- generates (or reuses) a local demo Ed25519 key at
  `data/pv-agent/keys/demo-key.json`;
- writes the canonical record at exactly the byte sequence that gets
  hashed;
- writes a manifest with per-file SHA-256;
- writes an anchor request stub.

## 3. Verify the bundle

```bash
./target/release/pv-agent verify \
  --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

Expected output:

```
PV evidence verification: PASS
record_id: pv-palermo-pv-001-20260515T101500Z
plant_id: palermo-pv-001
canonical_hash: sha256:...
checks:
  canonical_reproducible:           true
  canonical_hash_matches_manifest:  true
  signature_valid:                  true
  manifest_files_intact:            true
  anchor_request_matches:           true
  anchor_response_matches:          n/a (pending)
```

## 4. Inspect the bundle (no secrets)

```bash
./target/release/pv-agent inspect \
  --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

## 5. Optional: anchor to a local IPPANCENT staging endpoint

```bash
export IPPAN_ADMIN_TOKEN=...your-staging-token...
./target/release/pv-agent demo --plant palermo-1mw --submit-anchor --force
```

Or, against a fully-written config:

```bash
./target/release/pv-agent anchor-submit \
  --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z \
  --config examples/pv/pv-agent.example.toml \
  --submit-anchor
```

After successful submission, re-running `pv-agent verify` will also
include an `anchor_response_matches: true` check.

## 6. Determinism check

```bash
./target/release/pv-agent demo --plant palermo-1mw --force
sha256sum data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/canonical-record.json
```

Re-running the demo with the same canned input must produce the same
canonical bytes and therefore the same SHA-256. This is the property the
L1 anchor depends on.
