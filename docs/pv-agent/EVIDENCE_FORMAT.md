# Evidence format

## Goals

- The same logical input must always produce the same canonical bytes and
  the same SHA-256 commitment.
- An auditor with only the canonical bytes and the operator public key must
  be able to re-verify the signature and re-derive the L1 commitment
  without running `pv-agent`.

## Raw input

Decimal values come in as **strings** to avoid binary-float ambiguity in
the source data:

```json
{
  "plant_id": "palermo-pv-001",
  "timestamp": "2026-05-15T10:15:00Z",
  "interval_minutes": 15,
  "location": { "city": "Palermo", "country": "IT" },
  "source": { "source_type": "pv_simulator", "source_id": "nicola-palermo-sim-v1" },
  "telemetry": {
    "ghi_w_m2": "554",
    "ambient_temperature_c": "20.5",
    "dc_power_kw": "492.5",
    "ac_power_kw": "471.6",
    "meter_power_kw": "463.1",
    "performance_ratio": "0.859",
    "energy_since_start_kwh": "463.1"
  }
}
```

## Scaling rules

Raw decimal strings are converted to **scaled integers** using exact
integer arithmetic. No `f64` is ever used on the hashing path.

| Raw field                | Multiply by | Canonical field                    |
|--------------------------|-------------|------------------------------------|
| `ghi_w_m2`               | 1           | `ghi_w_m2`                         |
| `ambient_temperature_c`  | 1 000       | `ambient_temperature_milli_c`      |
| `dc_power_kw`            | 1 000       | `dc_power_w`                       |
| `ac_power_kw`            | 1 000       | `ac_power_w`                       |
| `meter_power_kw`         | 1 000       | `meter_power_w`                    |
| `performance_ratio`      | 1 000 000   | `performance_ratio_ppm`            |
| `energy_since_start_kwh` | 1 000       | `energy_since_start_wh`            |

Inputs with more fractional digits than the scale allows are **rejected**
(no rounding). Inputs that fail to parse as ASCII decimals are rejected.
Negative inputs are rejected for all fields except
`ambient_temperature_c`.

## Events

Allowed event types (others are rejected):

```
scheduled_maintenance
failure
module_cleaning
corrective_maintenance
replacement
```

Events are attached if they are open during the record interval, overlap
it, or ended within `events.lookback_minutes` before the interval start.

Attached events are sorted deterministically by:

1. `started_at`
2. `event_id`
3. `event_type`

## Canonical PV evidence record (`ippan.pv.production.v1`)

```json
{
  "schema": "ippan.pv.production.v1",
  "plant_id": "palermo-pv-001",
  "record_id": "pv-palermo-pv-001-20260515T101500Z",
  "timestamp": "2026-05-15T10:15:00Z",
  "interval_minutes": 15,
  "source": {
    "source_type": "pv_simulator",
    "source_id": "nicola-palermo-sim-v1"
  },
  "location": { "city": "Palermo", "country": "IT" },
  "telemetry": {
    "ghi_w_m2": 554,
    "ambient_temperature_milli_c": 20500,
    "dc_power_w": 492500,
    "ac_power_w": 471600,
    "meter_power_w": 463100,
    "performance_ratio_ppm": 859000,
    "energy_since_start_wh": 463100
  },
  "events": []
}
```

Record ID format: `pv-{plant_id}-{YYYYMMDDTHHMMSSZ}`.

## Canonical JSON encoder

- Object keys sorted lexicographically (byte order over UTF-8).
- No insignificant whitespace.
- No floating-point numbers (encoder rejects them).
- Strings JSON-escaped (`"`, `\`, control chars).
- UTF-8 output.

The output is *the* canonical bytes — `canonical-record.json` in the
evidence bundle contains exactly those bytes, byte-for-byte.

## Hashing

```
canonical_hash = "sha256:" || hex(SHA-256(canonical_bytes))
```

## Signing — `ippan.pv.evidence-envelope.v1`

```json
{
  "schema": "ippan.pv.evidence-envelope.v1",
  "record_id": "pv-palermo-pv-001-20260515T101500Z",
  "plant_id": "palermo-pv-001",
  "canonical_hash": "sha256:...",
  "signature": {
    "algorithm": "ed25519",
    "operator_key_ref": "key:plant-palermo-001",
    "public_key_b64": "...",
    "signature_value": "..."
  },
  "created_at": "2026-05-15T10:15:05Z"
}
```

The signature is computed over the **canonical bytes**, not the pretty
JSON envelope file. The envelope stores only the operator key reference,
the public key, and the signature — no private key material.

## Bundle layout

```
data/pv-agent/
  palermo-pv-001/
    records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/
      manifest.json
      canonical-record.json
      signature-envelope.json
      source-metadata.json
      events.json
      anchor-request.json
      anchor-response.json
      verification-report.json
```

`manifest.json` (schema `ippan.pv.evidence-manifest.v1`) records the
SHA-256 of each bundle file so any tampering surfaces immediately.
