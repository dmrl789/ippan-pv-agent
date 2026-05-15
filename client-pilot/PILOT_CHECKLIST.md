# Pilot checklist — Palermo 1MW

A 12-step walk from "binary not yet built" to "first real record
verified locally" — with an optional anchoring tail you can decide to
run later.

> **Client safety statement.** During the first pilot, pv-agent runs
> locally only. It does not submit data or anchors to IPPAN unless
> anchoring is explicitly enabled by configuration and command-line
> flag.

## Required (local-only)

```
[ ]  1. Build pv-agent locally
        → cargo build --release
        → binary appears at target/release/pv-agent (or .exe on Windows)

[ ]  2. Run Palermo demo
        → target/release/pv-agent demo --plant palermo-1mw
        → Expect: "Canonical record created: YES"
                  "Evidence bundle saved: YES"
                  "IPPAN L1 anchor submitted: NO" (this is the safe default)

[ ]  3. Verify the generated bundle
        → target/release/pv-agent verify --bundle <bundle-path>
        → Expect: "PV evidence verification: PASS"
                  "canonical_hash: sha256:ed47bc9df7...6256"

[ ]  4. Inspect the bundle
        → target/release/pv-agent inspect --bundle <bundle-path>
        → Expect: a human-readable summary that does NOT contain any
                  public-key bytes, signature bytes, or tokens.

[ ]  5. Confirm full PV data stays local
        → Open the bundle directory:
            data/pv-agent/palermo-pv-001/records/2026/05/15/<record-id>/
        → All 8 files exist locally.
        → Nothing has been uploaded anywhere.

[ ]  6. Confirm anchor request contains only the hash
        → Open the bundle file: anchor-request.json
        → Confirm it contains:
            "commitment": { "algorithm": "sha256", "hash": "sha256:..." }
          and does NOT contain any of:
            ghi_w_m2, dc_power, ac_power, meter_power, performance_ratio,
            energy_since_start, ambient_temperature.

[ ]  7. Connect simulator output file
        → Point pv-agent at one real 15-minute reading produced by your
          Palermo 1MW simulator. Either:
            (a) export the reading as JSON in the schema documented in
                examples/pv/palermo-telemetry.json, or
            (b) ask us for a small adapter script (Phase 2 work).

[ ]  8. Run one real 15-minute record
        → target/release/pv-agent run-once \
            --input <your-reading.json> \
            --events <your-events.json> \
            --config <your-config.toml>

[ ]  9. Verify the real record bundle
        → target/release/pv-agent verify --bundle <real-bundle-path>
        → Expect: PASS.
        → Note: the hash here will be different from the demo hash —
                that is correct, it depends on your real telemetry.
```

## Optional (staging, only when authorized)

```
[ ] 10. Configure a staging IPPAN endpoint
        → Edit your pv-agent.toml:
            [ippan]
            endpoint = "<staging endpoint URL>"
            anchor_path = "/v1/anchors"
            admin_token_env = "IPPAN_ADMIN_TOKEN"
            submit_anchors = true
        → Set the env var locally:
            (Linux)    export IPPAN_ADMIN_TOKEN=...
            (Windows)  $env:IPPAN_ADMIN_TOKEN = "..."

[ ] 11. Submit ONE staging anchor
        → target/release/pv-agent anchor-submit \
            --bundle <real-bundle-path> \
            --config <your-config.toml> \
            --submit-anchor
        → Expect: "Anchor submitted: YES" + a reference string.
        → The local bundle is updated with anchor-response.json.

[ ] 12. Retrieve and verify the L1 anchor
        → target/release/pv-agent anchor-status \
            --bundle <real-bundle-path> \
            --config <your-config.toml>
        → Then re-run verify:
            target/release/pv-agent verify --bundle <real-bundle-path>
        → Expect: "anchor_response_matches: true"
```

## Stop conditions

If any of the **required** steps (1–9) fail unexpectedly, **stop the
pilot** and contact us with:

- the exact command you ran;
- the full output (with no env-var values, no token);
- the relevant files from the bundle (`manifest.json`,
  `signature-envelope.json`, `verification-report.json`, and the
  `canonical-record.json` if you can share it).

We will not need your operator key file or any token to debug — only
the bundle and the command output.
