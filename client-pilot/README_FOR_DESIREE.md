# pv-agent — pilot package for the Palermo 1MW simulator

This folder is everything you need to run a first local pilot of
`pv-agent` next to your Palermo 1MW PV simulator. Nothing in this folder
contains keys, tokens, or production endpoints — it is safe to read and
to share inside your team.

## What pv-agent is

`pv-agent` is a small **local technical agent** for IPPAN. It is a single
binary that runs on a machine you already control — next to your PV
simulator, your meter bridge, your SCADA system, or your plant server.

Every 15 minutes, the agent:

1. **Reads PV production data** from the source you point it at (a file,
   a meter bridge, the simulator output).
2. **Attaches operational events** for that interval: scheduled
   maintenance, failures, module cleaning, corrective maintenance,
   replacement.
3. **Builds a deterministic evidence record** — the same input always
   produces the same bytes and the same hash.
4. **Signs the record** with an Ed25519 key tied to your plant.
5. **Stores the full evidence bundle locally** on your machine.
6. **Sends only the hash (commitment) to IPPAN L1** when anchoring is
   explicitly enabled. The full PV data never leaves the local machine.

Later, an auditor can verify, from your local bundle + the L1 anchor,
that the record existed at a specific time and was not modified.

## Simple picture

```
Palermo 1MW PV Simulator
        ↓
pv-agent
        ↓
Local evidence bundle
        ↓
Hash / commitment
        ↓
IPPAN L1 anchoring
        ↓
Auditor verification
```

## What this pilot is, and is not

**This pilot is:**
- A local installation and run of the agent on your machine.
- A walk through the Palermo 1MW demo so you can see exactly what an
  evidence bundle looks like.
- A connection to one record produced from your real simulator output,
  with local verification only.

**This pilot is not:**
- An anchoring run against any live IPPANCENT endpoint.
- A data-upload to IPPAN. None of your plant data will be sent.
- A commitment of any kind on your part. You can stop at any point.

> **Client safety statement.** During the first pilot, pv-agent runs
> locally only. It does not submit data or anchors to IPPAN unless
> anchoring is explicitly enabled by configuration and command-line
> flag.

## Where to start

| File | Purpose |
|------|---------|
| [QUICKSTART.md](QUICKSTART.md) | The 4 commands that produce and verify your first evidence bundle. |
| [WHAT_THIS_AGENT_DOES.md](WHAT_THIS_AGENT_DOES.md) | A plain-language explanation of what the agent does — and what it deliberately does NOT do. |
| [SAMPLE_OUTPUT.md](SAMPLE_OUTPUT.md) | The output you should see, with the known demo hash. Use this to confirm the pilot is healthy. |
| [INSTALL_WINDOWS.md](INSTALL_WINDOWS.md) | Step-by-step install on Windows. |
| [INSTALL_LINUX.md](INSTALL_LINUX.md) | Step-by-step install on Linux. |
| [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md) | 12-step checklist to walk the full pilot end-to-end. |

## Why this matters

> The agent may be proprietary, but the evidence must be independently
> verifiable.

You do not have to trust IPPAN, or us, or any specific software vendor.
The evidence bundle is small, documented, and verifiable with
off-the-shelf cryptography (SHA-256, Ed25519). An auditor can re-check
your records from the bundle alone.

That is the core of the IPPAN model for energy data: **plant data stays
local, IPPAN proves integrity.**
