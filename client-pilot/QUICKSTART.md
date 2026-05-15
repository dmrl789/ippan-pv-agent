# Quickstart — pv-agent pilot

Four commands. The first builds the binary; the rest produce and verify
an evidence bundle.

> **Client safety statement.** During the first pilot, pv-agent runs
> locally only. It does not submit data or anchors to IPPAN unless
> anchoring is explicitly enabled by configuration and command-line
> flag.

## 0. Prerequisite

Rust 1.74 or newer. See [INSTALL_WINDOWS.md](INSTALL_WINDOWS.md) or
[INSTALL_LINUX.md](INSTALL_LINUX.md) for one-line installs.

## 1. Build

```bash
cargo build --release
```

On success, the binary is at:

- Linux / macOS: `target/release/pv-agent`
- Windows:       `target\release\pv-agent.exe`

## 2. Run the Palermo 1MW demo

```bash
target/release/pv-agent demo --plant palermo-1mw
```

This produces a complete evidence bundle for a single 15-minute Palermo
1MW reading. The data is built in — no network access is needed.

You should see something like:

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

The "IPPAN L1 anchor submitted: NO" line is the safe default. The agent
will never send anything to IPPAN unless you explicitly opt in.

## 3. Verify the bundle

```bash
target/release/pv-agent verify --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

You should see:

```
PV evidence verification: PASS
record_id: pv-palermo-pv-001-20260515T101500Z
plant_id: palermo-pv-001
canonical_hash: sha256:c2150ae864a62d1bbdc284b81a55494656fff3b218838bab8fc848efa94d9171
checks:
  canonical_reproducible:           true
  canonical_hash_matches_manifest:  true
  signature_valid:                  true
  manifest_files_intact:            true
  anchor_request_matches:           true
  anchor_response_matches:          n/a (pending)
```

`PASS` means: the canonical bytes still hash to the recorded value, the
signature still verifies, every file in the manifest is intact, and the
anchor request — when you choose to send it — would carry the right
hash.

## 4. Inspect the bundle (human-readable)

```bash
target/release/pv-agent inspect --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

`inspect` is the human-friendly view. It deliberately shows you the
canonical hash, the operator key reference, and the telemetry, but
**not** the public key bytes, the signature bytes, or any token.

## Anchoring is off by default

By design, `pv-agent demo`, `pv-agent run-once`, and `pv-agent
anchor-submit` will not contact any IPPAN endpoint unless **all** of
the following are true:

1. The configuration sets `submit_anchors = true`, OR `--submit-anchor`
   is passed on the command line.
2. An environment variable holding a bearer token is set, and named in
   the `ippan.admin_token_env` field of the config.
3. The endpoint URL is reachable.

If any of these is missing, the agent refuses with a clear error and
the local evidence bundle is left intact.

## Where the files are

```
data/pv-agent/<plant_id>/records/<YYYY>/<MM>/<DD>/<record_id>/
├── manifest.json
├── canonical-record.json   ← exactly the bytes that are hashed
├── signature-envelope.json
├── source-metadata.json
├── events.json
├── anchor-request.json     ← commitment only — no telemetry
├── anchor-response.json    ← "pending" until you submit
└── verification-report.json
```

The hash you see in `pv-agent verify` is the SHA-256 of
`canonical-record.json`. You can re-verify it yourself with any
SHA-256 tool:

```bash
sha256sum data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/canonical-record.json
# → c2150ae864a62d1bbdc284b81a55494656fff3b218838bab8fc848efa94d9171
```

That number is the heart of the system. As long as nothing changes the
canonical bytes, that number is stable — and as soon as anything
changes, it changes too. That is what "deterministic evidence" means.
