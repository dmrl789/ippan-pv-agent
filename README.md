# IPPAN PV Agent

`pv-agent` is a local technical agent for photovoltaic plants.

It runs near a PV simulator, meter bridge, SCADA system, or plant
server. Every 15 minutes, it can read production data, attach
operational events such as maintenance or failures, create a
deterministic evidence record, sign it, store the full evidence
locally, and optionally anchor only the commitment/hash to IPPAN L1.

The full PV data does not need to be sent to IPPAN L1. IPPAN receives
only the verifiable commitment.

> The agent may be proprietary, but the evidence must be independently
> verifiable.

---

## 1. What is pv-agent?

A single small binary written in Rust. It is the bridge between a
photovoltaic plant (real or simulated) and IPPAN. It performs five
mechanical jobs, in order:

1. **Read** a PV production reading.
2. **Attach** the operational events that overlap the reading interval.
3. **Build** a canonical evidence record with strictly deterministic
   rules (string-decimal → scaled integers → sorted-key JSON →
   SHA-256).
4. **Sign** the canonical bytes with an Ed25519 operator key.
5. **Store** the complete evidence bundle locally, and *optionally*
   send only the hash commitment to IPPAN / IPPANCENT L1.

Plant data stays local. IPPAN proves integrity.

## 2. What problem does it solve?

A plant operator wants to be able to say, months or years later: *"this
15-minute reading existed at this exact moment, with this exact value,
and was not modified."* Without `pv-agent`, that claim is only as
trustworthy as the operator. With `pv-agent`:

- The reading is captured in a canonical, deterministic form. The same
  input always produces the same canonical bytes and the same SHA-256.
- The reading is signed by an operator key. The signature pins
  authorship.
- Only the commitment hash is anchored to IPPAN L1. Anyone with the
  local bundle + the L1 anchor + the operator public key can
  independently verify that nothing has been changed.

This is the same trust model that financial markets use for trade
evidence and that regulators use for audited reports — applied to PV
production data.

## 3. Workflow

```
Palermo 1MW PV Simulator / Meter Bridge
        ↓
pv-agent
        ↓
Canonical evidence record
        ↓
Signature + local evidence bundle
        ↓
Hash / commitment only
        ↓
IPPAN / IPPANCENT L1 anchoring
        ↓
Auditor verification
```

Each step is a separate command — the chain is not magic. You can stop
at any step, verify what was produced, and continue when you are
ready.

## 4. What data stays local

The complete evidence bundle is written to your local disk. It
contains everything needed for a future audit, and it never leaves the
machine unless you copy it elsewhere yourself.

A local bundle may contain:

- GHI / irradiance (W/m²)
- ambient temperature (°C)
- DC power (kW)
- AC power (kW)
- meter power (kW)
- performance ratio
- energy since start (kWh)
- maintenance events
- failure events
- module-cleaning events
- corrective-maintenance events
- replacement events
- the operator's signature envelope
- per-file manifest hashes
- the anchor request (commitment only)
- the anchor response (if you submitted one)
- the local verification report

> **IPPAN L1 does not need the full plant data.**

## 5. What is sent to IPPAN L1

When anchoring is enabled, `pv-agent` sends only a commitment/hash,
not the full telemetry.

Example anchor request body:

```json
{
  "workflow_type": "pv_production_evidence",
  "evidence_bundle_id": "pv-palermo-pv-001-20260515T101500Z",
  "commitment": {
    "algorithm": "sha256",
    "hash": "f1f117601eabbe70653267f5fb3a2d018b1e2429010c677c2f89284acca735f1"
  }
}
```

This is enough for IPPAN to prove that the evidence existed and was
not modified later, without storing the full PV data on L1.

## 6. Installation

Prerequisite: Rust 1.74 or newer.

### Windows

Install Rust from [rustup.rs](https://rustup.rs), then in a fresh
PowerShell window:

```powershell
cargo build --release
.\target\release\pv-agent.exe demo --plant palermo-1mw
```

The detailed install guide is in
[client-pilot/INSTALL_WINDOWS.md](client-pilot/INSTALL_WINDOWS.md).

### Linux / macOS

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo build --release
./target/release/pv-agent demo --plant palermo-1mw
```

The detailed install guide is in
[client-pilot/INSTALL_LINUX.md](client-pilot/INSTALL_LINUX.md).

## 7. Quickstart: Palermo 1MW demo

```bash
cargo build --release
target/release/pv-agent demo --plant palermo-1mw
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

> **By default, the demo does not submit any anchor to IPPAN.** The
> "IPPAN L1 anchor submitted: NO" line is the safe default. The agent
> will never reach the network unless you explicitly opt in (see §11).

## 8. Understanding the evidence bundle

`pv-agent demo` writes a directory under `data/pv-agent/<plant_id>/`.
For the bundled demo, that is:

```
data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/
├── manifest.json
├── canonical-record.json
├── signature-envelope.json
├── source-metadata.json
├── events.json
├── anchor-request.json
├── anchor-response.json
└── verification-report.json
```

| File | Purpose |
|------|---------|
| `canonical-record.json` | Deterministic PV evidence record. **These are exactly the bytes that get hashed.** |
| `signature-envelope.json` | Ed25519 signature proving who signed the record, plus the operator's public key |
| `manifest.json` | SHA-256 of every other file in the bundle (tamper detection) |
| `events.json` | The maintenance / failure / cleaning / replacement events attached to this interval |
| `source-metadata.json` | Non-hashing context: agent id, lookback window, source ids |
| `anchor-request.json` | The commitment-only request prepared for IPPAN L1 |
| `anchor-response.json` | Anchor result, or `{"status":"pending"}` if not yet submitted |
| `verification-report.json` | Result of `pv-agent verify` — written automatically each time you verify |

The hash in `canonical-record.json` is simply SHA-256 over the file's
bytes. You can re-verify it with any SHA-256 tool:

```bash
sha256sum data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/canonical-record.json
# → f1f117601eabbe70653267f5fb3a2d018b1e2429010c677c2f89284acca735f1
```

## 9. Verifying an evidence bundle

```bash
target/release/pv-agent verify --bundle <bundle-path>
```

Expected output:

```
PV evidence verification: PASS
```

Verification checks **six** invariants:

1. **canonical_reproducible** — parsing `canonical-record.json` back
   to a `Value` and re-canonicalizing yields the same bytes.
2. **canonical_hash_matches_manifest** — SHA-256 of the canonical
   bytes equals the recorded `canonical_hash` in the manifest.
3. **signature_valid** — the Ed25519 signature in
   `signature-envelope.json` verifies against the canonical bytes
   under the recorded public key.
4. **manifest_files_intact** — every file listed in `manifest.json`
   still hashes to its recorded SHA-256.
5. **anchor_request_matches** — `anchor-request.json` commits to the
   same hash as the manifest.
6. **anchor_response_matches** — if an anchor response is present
   (i.e. you submitted), it references the same hash. Pending
   responses are reported as `n/a (pending)`.

If **any** committed file is modified, verification fails. Try it:
edit a single byte of `canonical-record.json` and run `verify` again —
the four hashing-path checks will all flip to `false` and the CLI
will exit non-zero with `error: verification failed`.

## 10. Inspecting an evidence bundle

```bash
target/release/pv-agent inspect --bundle <bundle-path>
```

`inspect` is the human-readable view. It prints:

- plant ID
- record ID
- timestamp
- interval
- location and source
- canonical (integer) telemetry
- attached events
- canonical hash
- signature algorithm and operator key reference
- anchor status

It **does not** print:

- private keys
- admin or bearer tokens
- environment-variable values
- public-key bytes
- signature bytes

That makes `inspect` safe to paste into a ticket, share with an
auditor, or include in a regulator report.

## 11. Optional: submitting an anchor

> **Anchoring is disabled by default.** You must enable it twice —
> once in configuration and once at the command line — and you must
> have a bearer token available in an environment variable. If any of
> those is missing, the agent refuses.

To submit an anchor, configure an IPPAN endpoint and token. In your
`pv-agent.toml`:

```toml
[ippan]
endpoint = "https://devnet.ippan.net"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = true
```

Set the token in your shell (the env-var name must match
`admin_token_env`):

Linux / macOS:

```bash
export IPPAN_ADMIN_TOKEN="your-token-here"
```

Windows PowerShell:

```powershell
$env:IPPAN_ADMIN_TOKEN="your-token-here"
```

Submit:

```bash
target/release/pv-agent anchor-submit \
  --bundle <bundle-path> \
  --config examples/pv/pv-agent.example.toml
```

On success, `anchor-response.json` is updated and a future
`pv-agent verify` will additionally check that the L1 response
references the same canonical hash.

Important rules:

- Do not put the token inside the evidence bundle.
- Do not commit the token to git.
- Do not share the token.
- Anchoring sends **only** the commitment/hash — never the telemetry.

## 12. Checking anchor status

```bash
target/release/pv-agent anchor-status \
  --bundle <bundle-path> \
  --config examples/pv/pv-agent.example.toml
```

A successful status check should confirm that the L1 record matches
the local evidence commitment. The exact JSON shape depends on the
endpoint; `pv-agent` prints the response verbatim. After a successful
status check you can re-run `pv-agent verify` to get an updated
`anchor_response_matches: true` line.

## 13. PVSimulator Palermo 991 kWp data contract

This repository follows **IPPAN_DATA_SPECIFICATION v1.0**. That
document — owned by the simulator team — is the authoritative
description of the JSON that the Palermo 991 kWp PVSimulator (pvlib
+ Open-Meteo) emits at each 15-minute timestep, plus the structured
operational-events file that accompanies it. The agent's parser is
aligned to that contract; what follows is a quick reference.

### 13.1 What changed vs. the pre-v1.0 minimal schema

| Topic | Pre-v1.0 minimal | v1.0 data contract |
|---|---|---|
| Telemetry fields | 7 | **31** (POA, cell temp, DC field, kVA/kVAr, O&M state, …) |
| `location` | `city`, `country` | + `latitude`, `longitude`, `altitude_m` |
| `source` | `source_type`, `source_id` | + `model`, `weather_provider` |
| Active events on telemetry | absent | `active_event_ids: string[]` |
| Event type for inspections | `maintenance` | **`scheduled_maintenance`** (IEC 62446) |
| `affected_components` | flat strings (`"pv_string_01"`) | structured `{ type, id, strings_offline }` |
| `events.json` extras | absent | `status`, `impact`, `photos[]`, `root_cause`, `spare_part`, `insurance_claim`, `soiling_reset` |

The pre-v1.0 minimal payload is **rejected** by today's parser with
a clear "missing field" error. Operators upgrading must extend the
simulator output, not strip the schema down.

### 13.2 Telemetry payload (one file per 15-minute step)

Every numeric value is a **decimal string** — bare JSON numbers are
rejected at parse time so that canonical hashing never round-trips
through binary floats.

```json
{
  "plant_id": "palermo-pv-001",
  "timestamp": "2026-05-20T12:15:00Z",
  "interval_minutes": 15,
  "location": {
    "city": "Palermo",
    "country": "IT",
    "latitude": "38.1157",
    "longitude": "13.3615",
    "altitude_m": "14"
  },
  "source": {
    "source_type": "pv_simulator",
    "source_id": "pvlib-openmeteo-palermo-v2",
    "model": "pvlib-CEC-Sandia",
    "weather_provider": "open-meteo"
  },
  "telemetry": {
    "ghi_w_m2": "554",
    "dni_w_m2": "680",
    "dhi_w_m2": "120",
    "ambient_temperature_c": "20.5",
    "cell_temperature_c": "36.20",
    "humidity_pct": "55.0",
    "wind_speed_ms": "3.200",
    "precipitation_mm": "0.00",
    "cloudcover_pct": "15.0",
    "solar_elevation_deg": "52.300",
    "solar_azimuth_deg": "185.400",
    "poa_global_w_m2": "612",
    "poa_direct_w_m2": "480",
    "poa_diffuse_w_m2": "132",
    "dc_string_voltage_v": "372.10",
    "dc_string_current_a": "8.880",
    "dc_array_voltage_v": "372.10",
    "dc_power_kw": "492.5000",
    "ac_power_kw": "471.6000",
    "inverter_efficiency_pct": "95.780",
    "meter_power_kw": "463.1000",
    "apparent_power_kva": "472.5510",
    "reactive_power_kvar": "94.8200",
    "grid_voltage_v": "400.0",
    "grid_frequency_hz": "50.0",
    "performance_ratio": "0.8590",
    "capacity_factor_pct": "47.588",
    "energy_since_start_kwh": "463.100",
    "strings_available": "300",
    "derating_factor": "1.0000",
    "soiling_factor": "0.9985"
  },
  "active_event_ids": []
}
```

A complete example lives at
[examples/pv/palermo-telemetry.json](examples/pv/palermo-telemetry.json),
and an explicitly-invalid example showing the JSON-numbers form is
at
[examples/pv/palermo-telemetry-invalid-json-numbers.json](examples/pv/palermo-telemetry-invalid-json-numbers.json)
(referenced by tests to pin the rejection contract).

### 13.3 Events payload (`events.json`)

A JSON array of operational events. The agent attaches events to a
record when they overlap the current interval, when they ended
within `events.lookback_minutes` (recommended **240 min**), or when
they appear in `active_event_ids` on the telemetry payload.

The five canonical `event_type` values, all aligned to IEC 62446 /
the simulator's `events.yaml`:

- `scheduled_maintenance` — annual inspection, thermal imaging,
  firmware update, torque check (was `maintenance` pre-v1.0).
- `failure` — string open-circuit, connector fault, AFCI trip, fuse
  blow.
- `module_cleaning` — wet or dry cleaning. Carries
  `soiling_reset: true`.
- `corrective_maintenance` — planned repair following a detected
  fault (e.g. IGBT replacement). Carries `spare_part`.
- `replacement` — module / equipment replacement after hail, EoL.
  Carries `spare_part` and (when insured) `insurance_claim`.

`affected_components` is a list of structured objects:

```json
{ "type": "inverter", "id": "INV-03", "strings_offline": 30 }
{ "type": "string",   "id": "INV-05/STR-07", "strings_offline": 1 }
{ "type": "plant",    "id": "all", "strings_offline": 150 }
```

Component IDs are validated: `INV-01` through `INV-10`, each with
`STR-01` through `STR-30`. `photo_type` values are restricted to
`pre_intervention`, `in_progress`, `post_intervention`,
`fault_finding`, `diagnostics`.

A full example per event type is in
[examples/pv/events/](examples/pv/events/), and a representative
combined `events.json` is at
[examples/pv/palermo-events.json](examples/pv/palermo-events.json).

### 13.4 Running once with these inputs

```bash
target/release/pv-agent run-once \
  --input examples/pv/palermo-telemetry.json \
  --events examples/pv/palermo-events.json \
  --config examples/pv/pv-agent.example.toml
```

In Phase 2, this input can be supplied directly by an adapter to the
simulator output, a meter bridge API, a REST endpoint, a Modbus
gateway, or a SCADA export. The agent itself does not need to change
— only the upstream data feed.

## 14. Configuration

`pv-agent` reads a TOML config. A sensible starting point lives at
[examples/pv/pv-agent.example.toml](examples/pv/pv-agent.example.toml).
The full shape:

```toml
[agent]
agent_id = "pv-agent-palermo-001"
agent_type = "pv_plant_agent"
plant_id = "palermo-pv-001"
operator_key_ref = "key:plant-palermo-001"
production_mode = false

[storage]
base_dir = "data/pv-agent"

[key]
key_file = "data/pv-agent/keys/palermo-demo-key.json"

[ippan]
endpoint = "http://127.0.0.1:18181"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = false

[events]
lookback_minutes = 240
```

| Section | What it controls |
|---------|------------------|
| `[agent]` | Identity of this agent + plant. `production_mode = true` refuses demo keys unless `--allow-demo-key` is explicitly passed. |
| `[storage]` | Where evidence bundles are written. Should be on durable disk in production. |
| `[key]` | Path to the operator's Ed25519 key file. |
| `[ippan]` | L1 endpoint, the env var that holds the bearer token, and whether anchoring is enabled at all. `submit_anchors = false` is the safe default. |
| `[events]` | How far back (in minutes) `pv-agent` looks for recently-ended events to attach to the current interval. |

To write a fresh config from a template:

```bash
target/release/pv-agent init --out pv-agent.toml --plant-id palermo-pv-001
```

## 15. Key management

`pv-agent` signs each evidence record with an Ed25519 key. There are
two flavours:

- **Demo key** — generated locally by `pv-agent generate-demo-key`. The
  resulting file is tagged `is_demo: true`. Convenient for the pilot.
- **Operator key** — provisioned by the operator's own key tooling.
  Tagged `is_demo: false`. Required for production.

Generate a demo key:

```bash
target/release/pv-agent generate-demo-key \
  --out data/pv-agent/keys/demo-key.json \
  --key-ref "key:plant-palermo-001"
```

Rules — apply to both flavours, but more strictly to operator keys:

- Private keys must stay on the client machine. Treat them like SSH
  keys.
- Private keys must not be copied into evidence bundles. (The bundle
  only ever stores the *public* key and the signature.)
- Private keys must not be committed to git. The repository's
  `.gitignore` already excludes `keys/`, `*.pem`, `*.key`, `*.priv`,
  and `operator-key*`, `demo-key*`, `private-key*`.
- Demo keys must not be used in production.
- Production mode (`agent.production_mode = true`) refuses demo keys
  unless `--allow-demo-key` is explicitly passed.

## 16. Security and privacy guarantees

1. Full PV telemetry is stored **locally** or in an authorized IPPAN
   data space. IPPANCENT L1 receives only the commitment hash.
2. Private keys are never written into evidence bundles. Only the
   `operator_key_ref` and the public key are in the signature
   envelope.
3. Bearer tokens come from environment variables only, sent in the
   `Authorization` HTTP header only, and never logged, echoed, or
   persisted.
4. Failed anchor submissions never modify the local evidence bundle.
5. The same input always produces the same canonical hash. No float
   arithmetic on the hashing path.
6. The canonical record contains no private key, no token, no
   environment-variable value.
7. Anchor requests carry the commitment hash only — no telemetry, no
   events.
8. `pv-agent inspect` deliberately hides public-key bytes, signature
   bytes, and tokens.
9. The repository ships a secret-scan script
   ([scripts/secret-scan.sh](scripts/secret-scan.sh) and
   [scripts/secret-scan.ps1](scripts/secret-scan.ps1)) that refuses to
   ship if a file `git` would track contains a private-key marker, a
   bearer token, or a known sensitive field name.

## 17. Troubleshooting

### Verification fails

Possible causes:

- A file in the bundle was modified (intentionally or not).
- The wrong bundle path was used (`--bundle` should point at the
  directory containing `canonical-record.json`, not a parent).
- The signature envelope does not match the canonical record (rare —
  usually means a copy-paste edit of the envelope).
- The manifest hashes do not match (one or more files inside the
  bundle were altered after the manifest was written).

`pv-agent verify` prints which of the six checks failed; start from
the first `false` line.

### Anchor submission fails

Possible causes:

- Anchoring is disabled in config (`submit_anchors = false` and
  `--submit-anchor` was not passed).
- IPPAN endpoint is missing or unreachable.
- The token environment variable named in `ippan.admin_token_env` is
  not set, or is empty.
- The token is invalid (HTTP 401/403 from the endpoint).
- Network connection failed (timeout, DNS).
- Bundle verification failed before submission — `anchor-submit`
  refuses to send if `pv-agent verify` would not pass locally.

In all these cases the local bundle is left untouched. Re-running
`anchor-submit` after fixing the cause is safe.

### Demo does not run

Possible causes:

- Rust is not installed (`cargo --version` fails).
- The binary was not built (`cargo build --release` was skipped or
  failed).
- The command path is wrong (`pv-agent.exe` on Windows, `pv-agent` on
  Linux/macOS; both under `target/release/`).
- A previous bundle for the same record id already exists and
  `--force` was not used — re-run with `--force` to overwrite.

## 18. Current pilot limitations

This Phase 1 version is ready for a controlled local pilot. Current
limitations:

- Input is JSON / file-based. A direct SCADA or meter-bridge adapter
  is Phase 2 work.
- Live anchoring requires a configured IPPAN endpoint and bearer
  token — neither is provided in this package.
- Production key provisioning must be managed by the operator.
  `pv-agent` ships only `generate-demo-key`.
- The scheduler is documented (`pv-agent schedule`, plus systemd
  examples in `docs/pv-agent/`) but is not automatically enabled.
- On Windows, CLI output shows backslash path separators
  (`data\pv-agent\...`) while internal paths and verification use
  forward slashes — cosmetic only.

## 19. Client pilot documents

Additional client-facing documents are available in:

```
client-pilot/
```

Start with:

- [client-pilot/README_FOR_DESIREE.md](client-pilot/README_FOR_DESIREE.md) — what
  pv-agent is, in plain language, with the pilot framing.
- [client-pilot/QUICKSTART.md](client-pilot/QUICKSTART.md) — the four
  commands that produce and verify your first bundle.
- [client-pilot/PILOT_CHECKLIST.md](client-pilot/PILOT_CHECKLIST.md) — a
  12-step walk from "binary not built" to "first real record
  verified locally", with an opt-in staging-anchor tail.

Also useful:

- [client-pilot/WHAT_THIS_AGENT_DOES.md](client-pilot/WHAT_THIS_AGENT_DOES.md)
- [client-pilot/SAMPLE_OUTPUT.md](client-pilot/SAMPLE_OUTPUT.md)
- [client-pilot/INSTALL_WINDOWS.md](client-pilot/INSTALL_WINDOWS.md)
- [client-pilot/INSTALL_LINUX.md](client-pilot/INSTALL_LINUX.md)

And in-depth reference material lives under
[docs/pv-agent/](docs/pv-agent/):

- [ARCHITECTURE.md](docs/pv-agent/ARCHITECTURE.md) — module map
- [EVIDENCE_FORMAT.md](docs/pv-agent/EVIDENCE_FORMAT.md) — canonical schema, scaling rules, hashing
- [ANCHORING.md](docs/pv-agent/ANCHORING.md) — IPPAN L1 anchor client
- [SECURITY_PRIVACY.md](docs/pv-agent/SECURITY_PRIVACY.md) — what stays local, what goes to L1
- [DEMO_PALERMO_1MW.md](docs/pv-agent/DEMO_PALERMO_1MW.md) — guided demo walkthrough
- [CLIENT_DELIVERY.md](docs/pv-agent/CLIENT_DELIVERY.md) — packaging & install for energy clients
- [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md) — pre-delivery gate

## 20. Next pilot steps

1. **Run the Palermo 1MW demo locally** to confirm the binary
   produces the expected canonical hash:
   `sha256:f1f117601eabbe70653267f5fb3a2d018b1e2429010c677c2f89284acca735f1`.
2. **Produce one real 15-minute record** from your own simulator
   output via `pv-agent run-once`. Verify it locally.
3. **Walk the pilot checklist** in
   [client-pilot/PILOT_CHECKLIST.md](client-pilot/PILOT_CHECKLIST.md)
   end-to-end. Stop at step 9 — that is the safe stopping point.
4. **Optionally**, after explicit authorization, configure a staging
   IPPAN endpoint and submit a single canary anchor (steps 10–12 of
   the checklist).
5. **Plan the Phase 2 adapter** for direct simulator / meter-bridge
   input.

---

## Licensing

`pv-agent` is intended as part of the IPPAN AgentOS client-side
stack. It can be distributed commercially, source-available under
NDA, or partially open-sourced in the future. The recommended model
for early pilots is proprietary or source-available, while keeping
the evidence format and verifier transparent enough for independent
audit.

No open-source license is attached at this time.
