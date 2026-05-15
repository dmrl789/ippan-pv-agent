# Security and privacy

## Hard rules

1. Full PV telemetry is stored **locally** or in an authorized IPPAN data
   space. IPPANCENT L1 receives only the commitment hash.
2. **Private keys are never written into evidence bundles.** Only the
   `operator_key_ref` and the public key are in the signature envelope.
3. Admin / bearer tokens come from environment variables and are never
   logged, echoed, or persisted.
4. Failed anchor submissions never destroy local evidence.
5. Evidence bundles remain verifiable offline — `pv-agent verify` does
   not require network access.
6. The same input always produces the same canonical hash.
7. The canonical record contains no private key, no token, no secret.
8. Demo keys are clearly marked `is_demo = true`. Production mode
   (`agent.production_mode = true`) refuses demo keys unless
   `--allow-demo-key` is explicitly passed.
9. Anchor requests carry the commitment hash only — no telemetry, no
   events.
10. Error messages are written without secrets in them.

## Threat model (in scope)

- A future operator wants to silently rewrite a past production reading.
  → blocked: rewriting `canonical-record.json` changes its SHA-256, which
  invalidates the manifest and the L1 anchor.
- A storage administrator wants to back-date or forge events.
  → blocked: events are part of the canonical record. Any change shifts
  the canonical hash.
- A plant operator wants to forge readings without the signing key.
  → blocked: the signature envelope requires the Ed25519 private key.
- An auditor wants to verify an old bundle without trusting the agent.
  → supported: the canonical schema, hashing, and signature scheme are
  documented and reproducible with off-the-shelf libraries.

## Threat model (out of scope, for now)

- Compromise of the operator's signing key. (Mitigation: rotate
  `operator_key_ref`; future phase will document key rotation evidence.)
- Compromise of the IPPAN endpoint itself. (Mitigation: independent
  verifiers can re-derive the commitment from local bundles.)
- Physical tampering with the simulator/meter bridge upstream of the
  agent. The agent records what it is given; upstream attestation is a
  separate problem.

## Secret-scan policy

The repository ships with a tiny secret-scan helper at
[scripts/secret-scan.sh](../../scripts/secret-scan.sh) (and a PowerShell
counterpart at [scripts/secret-scan.ps1](../../scripts/secret-scan.ps1)).
It looks for the following patterns inside committed evidence bundles,
example data, and configuration files:

```
BEGIN PRIVATE KEY
admin_token=
bearer
IPPAN_ADMIN_TOKEN=
operator_private
private_key
```

These strings are allowed to appear inside documentation **as
explanatory text**, but not in committed evidence or configuration.
