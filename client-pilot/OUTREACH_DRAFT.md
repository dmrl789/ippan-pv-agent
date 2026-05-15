# Outreach draft — Nicola, Palermo 1MW pilot

A polished message ready to send. Edit details (greeting, sign-off,
contact information) before sending; the body is final.

---

Hi Nicola,

we have prepared the first version of the **IPPAN PV Agent** for the
Palermo 1MW simulator.

The agent runs locally near the simulator. It can take the 15-minute
PV production data, create a deterministic evidence record, sign it,
store the full evidence locally, and later anchor only the
hash/commitment to IPPAN.

The important point is that the full plant data does **not** need to
be sent to IPPAN L1. The data stays local; IPPAN is used to prove
that the evidence existed and was not modified later.

For the first pilot, anchoring is disabled by default. We can first
run everything locally: generate a demo bundle, verify it, inspect
it, and then connect it to your simulator output.

The demo has a deterministic self-check: the same Palermo 1MW demo
should always produce the same canonical hash:

`sha256:ed47bc9df77ad56dc0b11f05d365b1a79adaec1f20563bcfa5b37496ca236256`

This means you can verify that the software is producing the same
evidence record before any anchoring or external submission is
enabled.

The repository now includes:

- a full README with installation and usage instructions;
- a `client-pilot/` folder with simplified pilot instructions;
- a Palermo 1MW demo;
- verification commands;
- inspection commands;
- optional anchoring instructions for a later staging test.

If helpful, we can provide either the repository or a built binary
together with `client-pilot/README_FOR_DESIREE.md`. On a fresh
machine, the first local pilot should go from installation to:

`PV evidence verification: PASS`

in about 20 minutes.

The next step is to test it locally with your simulator output file.
