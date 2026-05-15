# pv-agent — overview

The PV Agent is the bridge between a photovoltaic plant and IPPAN.

It can run beside a simulator, meter bridge, SCADA system, or real plant
server. Every 15 minutes it reads production data, attaches operational
events such as maintenance, failures, cleaning, or replacement, creates a
deterministic evidence record, signs it, stores the full evidence locally
or in an IPPAN data space, and anchors only the commitment to IPPAN L1.

IPPAN is not used as a simple database. It becomes a verification
infrastructure: the agent produces evidence, IPPAN anchors the proof, and
an auditor can later verify that the data was not modified.

## Why a standalone repository

`pv-agent` is client-deliverable software. It is intentionally separated
from the main IPPAN AgentOS monorepo and the IPPANCENT L1 repository so
it can be:

- packaged and shipped to energy clients without exposing the rest of the stack;
- licensed independently (proprietary, source-available, or open-sourced later);
- audited by independent auditors against a small, self-contained code base.

Integration with IPPAN happens through stable interfaces only:

- the documented canonical evidence format;
- the documented signature and verification format;
- the IPPANCENT HTTP anchor endpoint;
- optional AgentOS evidence/data APIs in later phases.

## Why this is not necessarily a full AI agent

`pv-agent` is a deterministic software agent, not an LLM-driven agent. The
hashing path is purely mechanical: read input → scale decimals to integers
→ sort canonical fields → SHA-256 → Ed25519 sign → write evidence. The
same input always produces the same hash. This is what makes the evidence
independently verifiable.

A future phase may layer LLM/agentic capabilities on top (anomaly
explanation, narrative summaries), but those layers are explicitly **not**
on the canonical hashing path.

## Why the agent can be proprietary while the evidence remains independently verifiable

The verifier only needs:

1. the documented canonical schema;
2. a SHA-256 implementation;
3. an Ed25519 verifier;
4. the public key bound to `operator_key_ref`.

That is enough to reproduce the canonical bytes from a bundle and check
the signature and L1 anchor — without ever reading the agent's source
code.

## Where to next

- [ARCHITECTURE.md](ARCHITECTURE.md) — module map.
- [EVIDENCE_FORMAT.md](EVIDENCE_FORMAT.md) — canonical schema, scaling rules, hashing.
- [ANCHORING.md](ANCHORING.md) — IPPAN L1 anchor client.
- [SECURITY_PRIVACY.md](SECURITY_PRIVACY.md) — what stays local, what goes to L1.
- [DEMO_PALERMO_1MW.md](DEMO_PALERMO_1MW.md) — guided demo walkthrough.
- [CLIENT_DELIVERY.md](CLIENT_DELIVERY.md) — packaging & install for energy clients.
