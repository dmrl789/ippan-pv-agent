# IPPAN / IPPANCENT L1 anchoring

`pv-agent` anchors **only the commitment hash** to IPPAN L1. Full
telemetry never leaves the local bundle.

## Anchor request (`ippan.l1.anchor.request.v1`)

```json
{
  "schema": "ippan.l1.anchor.request.v1",
  "workflow_type": "pv_production_evidence",
  "operator_key_ref": "key:plant-palermo-001",
  "evidence_bundle_id": "pv-palermo-pv-001-20260515T101500Z",
  "commitment": {
    "algorithm": "sha256",
    "hash": "sha256:..."
  }
}
```

Note: there is no telemetry, no events, no PR or kWh, no operator
identity beyond the public `operator_key_ref`.

## Expected response

```json
{
  "status": "submitted",
  "reference": "ippan-l1-anchor:...",
  "anchor_hash": "...",
  "sequence": 123,
  "submitted_at_logical": 2694001
}
```

The exact field set is determined by the IPPANCENT endpoint; `pv-agent`
stores the response verbatim in `anchor-response.json`.

## Defaults

`submit_anchors = false` by default. `pv-agent demo` and `pv-agent
run-once` never submit unless one of:

- `submit_anchors = true` in config; OR
- `--submit-anchor` is passed on the CLI.

## Token handling

Bearer tokens come from a configured environment variable
(`ippan.admin_token_env`). The token:

- is read from the environment at submission time;
- is sent in the `Authorization: Bearer ...` HTTP header only;
- is **never** written to any file in the bundle;
- is **never** logged or echoed to stdout/stderr.

If `--submit-anchor` is requested but the env var is unset, the agent
refuses to submit.

## Failure handling

If submission fails:

- the local evidence bundle is **not modified**;
- `anchor-response.json` remains at `status=pending`;
- the user receives an error message describing the failure (HTTP status,
  transport error) without the token;
- re-running `pv-agent anchor-submit` will retry.

If submission succeeds, the bundle's `anchor-response.json` is updated.
Subsequent submissions are refused unless `--force` is passed.

## Status retrieval

`pv-agent anchor-status --bundle <path> --config <path>` issues a GET
against `{endpoint}{anchor_path}/{reference}` using the same token and
returns the response JSON verbatim. Endpoints that don't expose a status
GET will return their own error.
