# AgentOS-compatible evidence bundle export

`pv-agent export` emits a single JSON bundle (`ippan.pv.evidence_bundle.v1`)
that IPPAN AgentOS Energy can import, validate and preview. This is the bridge:

```
pv-agent (local) → ippan.pv.evidence_bundle.v1 → AgentOS Energy
POST /api/energy/import-pv-agent-bundle (validate + preview)
```

## Command

```bash
pv-agent export --bundle <existing-bundle-dir> --format agentos --out ./agentos-bundle.json
```

- `--bundle` — a local evidence bundle directory previously produced by
  `pv-agent demo` / `run-once` (contains `manifest.json`,
  `signature-envelope.json`, `canonical-record.json`, `source-metadata.json`,
  and optionally `anchor-response.json`).
- `--format` — one of:
  - `agentos` (default) — the bare `ippan.pv.evidence_bundle.v1` bundle.
  - `agentos-import` — the `{ bundle, signed_payload }` wrapper that also ships
    the exact signed canonical bytes so AgentOS can verify the signature
    (see [Verifiable import](#verifiable-import-format-agentos-import) below).
- `--out` — the JSON file to write.

## What the bundle contains

| Field | Source |
|-------|--------|
| `schema_version` | `ippan.pv.evidence_bundle.v1` |
| `agent.agent_id` | `source-metadata.json` |
| `agent.agent_version` | the pv-agent crate version |
| `agent.public_key` | the envelope's base64 Ed25519 public key |
| `plant.asset_ref` | **pseudonymous** `earef:<sha256("ippan-energy-asset:"+plant_id)[:32]>` |
| `plant.technology` | `solar_pv` |
| `plant.location_ref` | coarse `city, country` only (no coordinates) |
| `period.start/end/interval_minutes` | from the canonical record (end = start + interval) |
| `summary.record_count` | `1` (one canonical record per bundle) |
| `summary.total_energy_kwh` | derived from `energy_since_start_wh / 1000` |
| `summary.anomalies_count` | number of attached events |
| `hashes.data_hash` / `commitment_hash` | the canonical record hash (`sha256:<hex>`) — they coincide for a single-record bundle |
| `hashes.canonicalization` | the canonicaliser label |
| `signature.{algorithm,signer,signature}` | the existing Ed25519 signature over the canonical record bytes |
| `anchor` | local state only — defaults to `mode: dry_run`, `submission_status: dry_run_only` (a real reference appears only if an anchor response exists) |
| `files` | pointers within the bundle directory, incl. `signed_payload` = `canonical-record.json` |

## What is never included

Raw high-frequency telemetry rows, the raw plant id (only its pseudonym),
precise coordinates, customer/operator names, or private keys. The export
submits nothing to L1.

## Signature & AgentOS verification

The bundle carries the agent's Ed25519 signature over the **canonical record
bytes**. Imported on its own (bare `agentos` format), AgentOS reports
`signature_status: present_not_verified` — it has the signature metadata but
not the bytes that were signed. The `files.signed_payload` pointer
(`canonical-record.json`) names exactly which bytes were signed.

## Verifiable import (`--format agentos-import`)

To let AgentOS reach `signature_status: verified`, export the **import
wrapper**, which carries the exact signed bytes alongside the bundle:

```bash
pv-agent export --bundle <bundle-dir> --format agentos-import --out ./agentos-import.json
```

Output shape (matches the AgentOS import route
`POST /api/energy/import-pv-agent-bundle`):

```jsonc
{
  "bundle": { /* ippan.pv.evidence_bundle.v1 — identical to the bare export */ },
  "signed_payload": {
    "encoding": "utf8",                 // or "base64" if the bytes are not valid UTF-8
    "canonical_bytes": "…",             // the EXACT bytes that were signed
    "content_type": "application/json",
    "description": "canonical-record.json"
  }
}
```

Key guarantees:

- **Exact bytes, no regeneration.** `signed_payload.canonical_bytes` is the
  verbatim content of the bundle's `canonical-record.json` — the precise bytes
  the agent signed. The agent does not reformat, re-canonicalize, or regenerate
  them. AgentOS verifies them **verbatim**.
- **`verified` only on success.** AgentOS verifies the Ed25519 signature
  (`signature.signer` / `signature.signature`) against the supplied bytes and
  reports `verified` only when it succeeds, `failed` when it does not.
- **The signed bytes are the full canonical record.** Because verification
  requires the exact signed input, the wrapper necessarily includes the
  canonical record contents (plant id + telemetry). This is inherent to
  signature verification — use the bare `agentos` format for the
  validate + preview path when you do not need verification.
- **Encoding.** The canonical record is valid UTF-8 by construction, so it
  ships as `encoding: "utf8"`; the agent falls back to `base64` only if the
  bytes were ever not valid UTF-8.

A real, verifying wrapper example (built from the fictional Palermo demo) is at
[`examples/agentos/pv-agent-import.example.json`](../../examples/agentos/pv-agent-import.example.json).

> The bare `agentos` format is unchanged, so existing consumers are
> unaffected. This milestone adds **no** persistence, anchoring, L1 contact,
> SCADA/inverter integration, or live submission.

## Hash format

Hashes are emitted as `sha256:<64-hex>`. AgentOS accepts both `sha256:<64hex>`
and bare `<64hex>` and normalises to bare lowercase hex internally (it derives
`pack_id = ep-pv-<commitment[:16]>`).

## Testing against AgentOS

The AgentOS route is auth-gated, so an unauthenticated request returns `401`:

```bash
# Expected: 401 (auth required) — this only checks the route exists.
curl -s -o /dev/null -w "%{http_code}\n" \
  -X POST "https://agentos.ippan.com/api/energy/import-pv-agent-bundle" \
  -H "Content-Type: application/json" \
  --data-binary @examples/agentos/pv-agent-bundle.example.json
```

Authenticated testing requires a valid AgentOS session. Do not include
credentials in scripts or commits.
