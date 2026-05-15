# Sample output — pv-agent Palermo 1MW demo

This is the output you should see when running the bundled Palermo 1MW
demo. Use this as a reference: if your local run produces the **same
canonical hash**, the pilot is healthy.

> **All output below is sample / demonstration output.** It was produced
> from the canned Palermo 1MW demo data inside the repository. Real
> plant runs will produce different telemetry values and different
> hashes.

## 1. `pv-agent demo --plant palermo-1mw`

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

## 2. `pv-agent verify --bundle <bundle-path>`

```
PV evidence verification: PASS
record_id: pv-palermo-pv-001-20260515T101500Z
plant_id: palermo-pv-001
canonical_hash: sha256:ed47bc9df77ad56dc0b11f05d365b1a79adaec1f20563bcfa5b37496ca236256
checks:
  canonical_reproducible:           true
  canonical_hash_matches_manifest:  true
  signature_valid:                  true
  manifest_files_intact:            true
  anchor_request_matches:           true
  anchor_response_matches:          n/a (pending)
```

The canonical hash for the bundled Palermo 1MW demo is always:

```
sha256:ed47bc9df77ad56dc0b11f05d365b1a79adaec1f20563bcfa5b37496ca236256
```

If your local run produces a different hash, something is wrong with the
input — the canned demo is fully deterministic and must produce this
exact value on every machine, every operating system, every run.

## 3. `pv-agent inspect --bundle <bundle-path>`

```
Bundle:           data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
Plant ID:         palermo-pv-001
Record ID:        pv-palermo-pv-001-20260515T101500Z
Timestamp:        2026-05-15T10:15:00Z
Interval:         15 minutes
Location:         Palermo, IT
Source:           pv_simulator / nicola-palermo-sim-v1

Telemetry (canonical, integer):
  ghi_w_m2                       554
  ambient_temperature_milli_c    20500
  dc_power_w                     492500
  ac_power_w                     471600
  meter_power_w                  463100
  performance_ratio_ppm          859000
  energy_since_start_wh          463100

Attached events:  1
  - evt-20260515-001 [module_cleaning] started 2026-05-15T08:00:00Z

Canonical hash:   sha256:ed47bc9df77ad56dc0b11f05d365b1a79adaec1f20563bcfa5b37496ca236256
Signature:        algorithm=ed25519 key_ref=key:plant-palermo-001
Anchor status:    pending
```

Notice what `inspect` shows and what it deliberately omits:

| Shown                | Hidden                          |
|----------------------|---------------------------------|
| Plant / record IDs   | Public-key bytes                |
| Timestamp, interval  | Signature bytes                 |
| Canonical telemetry  | Any environment-variable values |
| Attached events      | Any bearer tokens               |
| Canonical hash       | Any private-key material        |
| Operator key ref     |                                 |
| Anchor status        |                                 |

This means `inspect` is safe to share with auditors, regulators, or
your own internal teams. It tells you everything you need to read the
record, and nothing you should not be reading on a console.

## 4. Independent verification (no `pv-agent` needed)

The canonical hash is just SHA-256 of `canonical-record.json`. You can
verify it with any SHA-256 tool:

```bash
sha256sum data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/canonical-record.json
# Expected:
# ed47bc9df77ad56dc0b11f05d365b1a79adaec1f20563bcfa5b37496ca236256
```

PowerShell:

```powershell
Get-FileHash data\pv-agent\palermo-pv-001\records\2026\05\15\pv-palermo-pv-001-20260515T101500Z\canonical-record.json -Algorithm SHA256
# Hash : C2150AE864A62D1BBDC284B81A55494656FFF3B218838BAB8FC848EFA94D9171
```

This is the property auditors rely on: the verifier needs only the
canonical record file, a SHA-256 implementation, an Ed25519 verifier,
and your public key. Nothing else.
