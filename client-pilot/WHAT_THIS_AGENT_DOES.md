# What pv-agent does (and what it does not)

This document is for anyone who needs to understand `pv-agent` without
reading code.

> **Client safety statement.** During the first pilot, pv-agent runs
> locally only. It does not submit data or anchors to IPPAN unless
> anchoring is explicitly enabled by configuration and command-line
> flag.

## In one sentence

`pv-agent` creates a verifiable proof that a specific PV record
existed at a specific time and was not modified later.

That is all.

## What it does NOT do

- **The agent does not replace the simulator.** Your existing PV
  simulator continues to produce telemetry. `pv-agent` is downstream of
  it.
- **The agent does not replace the meter.** Real-time SCADA, alarming,
  control, and metering remain on the systems that already do those
  jobs.
- **The agent does not send all plant data to IPPAN.** Plant data stays
  on your machine. What goes to IPPAN L1 — only when you explicitly
  enable it — is a fixed-size cryptographic commitment (a single
  SHA-256 hash plus the bundle's identifier).
- **The agent does not require the cloud to work.** The Palermo demo
  runs offline. Local verification runs offline. Anchoring is the only
  step that needs the network.
- **The agent does not store secrets in the evidence bundle.** No
  private key, no bearer token, no environment variable, no admin
  credential is ever written into a bundle file.

## What it does do

- **Reads PV production data** (GHI, DC/AC/meter power, performance
  ratio, energy since start, ambient temperature) once per interval
  (default: 15 minutes).
- **Attaches operational events** that overlap or recently preceded the
  interval — scheduled maintenance, failures, module cleaning,
  corrective maintenance, replacement.
- **Builds a canonical evidence record** with strictly deterministic
  rules: decimal inputs are parsed as strings and converted to scaled
  integers (no floating-point arithmetic on the hashing path), object
  keys are sorted, event order is fixed, timestamps require an ISO 8601
  `Z` suffix.
- **Hashes the canonical bytes** with SHA-256. That hash is the
  *commitment*.
- **Signs the canonical bytes** with an Ed25519 operator key. The
  signature, the operator key reference, and the public key all go
  into the bundle. The private key does not.
- **Writes an atomic, append-only evidence bundle** to local storage.
- **Optionally submits the commitment** (and only the commitment) to
  IPPAN / IPPANCENT L1.

## Why the commitment-only design matters

A naive design would push all telemetry to a shared system. That is
both heavy (cost, throughput, privacy) and unnecessary. The agent
instead does the opposite: keep the data, share the *proof*.

- The proof is small (a fixed-size hash plus a few identifiers).
- The proof is independent (an auditor with the local bundle + the L1
  anchor can re-check everything; they do not need IPPAN to cooperate).
- The proof is precise (any change to the bundle changes the hash; the
  signature pins the proof to your operator key).

## What an auditor does later

An auditor receives:

1. The bundle directory (~10 small JSON files).
2. The public key associated with your `operator_key_ref`.
3. Optionally, the L1 anchor reference.

They run `pv-agent verify --bundle <path>` (or any compatible
independent verifier) and they get either `PASS` or `FAIL` — and
exactly which check failed if it failed.

## Why proprietary code is acceptable here

> The agent may be proprietary, but the evidence must be independently
> verifiable.

The verifier is part of the same binary, but the *format* is
documented (see `docs/pv-agent/EVIDENCE_FORMAT.md` in the source repo).
Anyone with the format spec, SHA-256, and Ed25519 can build their own
verifier. The agent could be closed source forever and the evidence
would still be open to audit. That is the point.
