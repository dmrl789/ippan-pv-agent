# ippan-pv-agent

`pv-agent` is a client-side photovoltaic plant agent for IPPAN.

It runs beside a PV simulator, meter bridge, SCADA system, or plant server.
Every 15 minutes (configurable) it reads production data, attaches operational
events (maintenance, failure, cleaning, replacement), builds a **deterministic
canonical evidence record**, signs it with Ed25519, stores the complete
evidence bundle locally (or in an authorized IPPAN data space), and anchors
**only the commitment hash** to IPPAN / IPPANCENT L1.

> The agent may be proprietary, but the evidence must be independently
> verifiable.

`ippan-pv-agent` is intentionally separated from the main IPPAN AgentOS and
IPPANCENT repositories so it can be packaged, licensed, audited, and
delivered to clients independently.

## What it does

```
PV Simulator / Meter Bridge / Plant Server
        ↓
Local pv-agent
        ↓
Canonical deterministic PV evidence record
        ↓
Signature + hash + evidence bundle
        ↓
Local storage or IPPAN data space
        ↓
IPPANCENT L1 anchoring of commitment only
        ↓
Auditor verification
```

IPPAN is **not used as a database**. It is a *verification infrastructure*:
the agent produces evidence, IPPAN anchors the proof, and an auditor can
later check that the data was not modified.

## Quickstart

```bash
# Build (release binary lands at target/release/pv-agent)
cargo build --release

# One-shot deterministic demo with the bundled Palermo 1MW plant
./target/release/pv-agent demo --plant palermo-1mw

# Verify the bundle that was just created
./target/release/pv-agent verify --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

The demo never contacts the network unless you explicitly pass
`--submit-anchor` and have a configured IPPAN endpoint.

## CLI commands

| Command | Purpose |
|---------|---------|
| `pv-agent demo --plant palermo-1mw` | Build one deterministic Palermo demo bundle |
| `pv-agent run-once --input ... --events ... --config ...` | Build one bundle from input files |
| `pv-agent verify --bundle <path>` | Verify a local bundle |
| `pv-agent inspect --bundle <path>` | Print a human-readable bundle summary (no secrets) |
| `pv-agent anchor-submit --bundle <path> --config <path>` | Submit the commitment hash to IPPAN L1 |
| `pv-agent anchor-status --bundle <path> --config <path>` | Fetch L1 anchor status |
| `pv-agent init --out pv-agent.toml` | Write a default config |
| `pv-agent generate-demo-key --out keys/demo-key.json` | Generate a demo Ed25519 key |
| `pv-agent schedule --config ... --input ...` | Internal scheduler loop |

## Documentation

- [docs/pv-agent/README.md](docs/pv-agent/README.md) — overview
- [docs/pv-agent/ARCHITECTURE.md](docs/pv-agent/ARCHITECTURE.md) — module map
- [docs/pv-agent/EVIDENCE_FORMAT.md](docs/pv-agent/EVIDENCE_FORMAT.md) — canonical schema, scaling rules, hashing
- [docs/pv-agent/ANCHORING.md](docs/pv-agent/ANCHORING.md) — IPPAN L1 anchor client
- [docs/pv-agent/SECURITY_PRIVACY.md](docs/pv-agent/SECURITY_PRIVACY.md) — what stays local, what goes to L1
- [docs/pv-agent/DEMO_PALERMO_1MW.md](docs/pv-agent/DEMO_PALERMO_1MW.md) — guided demo walkthrough
- [docs/pv-agent/CLIENT_DELIVERY.md](docs/pv-agent/CLIENT_DELIVERY.md) — packaging & install for energy clients

## Licensing

`pv-agent` is intended as part of the IPPAN AgentOS client-side stack. It can
be distributed commercially, source-available under NDA, or partially
open-sourced in the future. The recommended model for early pilots is
proprietary or source-available, while keeping the evidence format and
verifier transparent enough for independent audit.

No open-source license is attached at this time.
