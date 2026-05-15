# Release checklist — ippan-pv-agent

Run through this list before delivering a client package or cutting a
versioned release. Items must be checked **in order** and from a clean
worktree.

## A. Clean worktree

- [ ] `git status` reports a clean tree.
- [ ] No `data/`, no `target/`, no `*.pem`, no `*.key` tracked.
- [ ] No real endpoint URL, admin token, or production plant ID in any
      committed file.
- [ ] `LICENSE` is absent unless the release has been explicitly cleared
      for open-source distribution.

## B. Tests

- [ ] `cargo fmt --check` passes (or formatting is intentionally skipped).
- [ ] `cargo clippy --all-targets -- -D warnings` passes (or known
      warnings are documented).
- [ ] `cargo test` passes — every test, including integration tests.
- [ ] `cargo build --release` succeeds without warnings on the target
      platforms (Linux x86_64, Windows x86_64, macOS arm64 — at minimum
      whichever the client will run).

## C. Determinism walk

- [ ] `pv-agent demo --plant palermo-1mw` succeeds.
- [ ] `pv-agent verify --bundle <created-bundle>` prints
      `PV evidence verification: PASS`.
- [ ] Re-running the demo with `--force` produces the SAME canonical
      hash (the bundled `canonical-record.json` file is byte-identical).
- [ ] `pv-agent run-once --input examples/pv/palermo-telemetry.json
      --events examples/pv/palermo-events.json --config
      examples/pv/pv-agent.example.toml --force` produces the SAME
      canonical hash as `pv-agent demo`.
- [ ] Independently computed `sha256` over the canonical-record file
      matches the `canonical_hash` reported by `pv-agent verify`.

## D. Security walk

- [ ] `bash scripts/secret-scan.sh` (or PowerShell equivalent) prints
      `secret-scan: clean`.
- [ ] `pv-agent inspect --bundle <bundle>` does NOT print: public key
      bytes, signature bytes, bearer tokens, env-var values, or any
      `secret_seed_b64` material.
- [ ] No bundle file contains `secret_seed_b64`, `BEGIN PRIVATE KEY`,
      `Bearer `, or `IPPAN_ADMIN_TOKEN=`.
- [ ] Tampering with `canonical-record.json` makes `pv-agent verify`
      exit non-zero (manual check, then revert).
- [ ] With `IPPAN_ADMIN_TOKEN` unset, `pv-agent anchor-submit
      --submit-anchor` refuses to send.

## E. Documentation

- [ ] README explains build + first-run.
- [ ] `docs/pv-agent/CLIENT_DELIVERY.md` is current for the client's
      install/path conventions (Unix `/etc/ippan/`, Windows
      `%ProgramData%\IPPAN\`, etc.).
- [ ] `docs/pv-agent/EVIDENCE_FORMAT.md` matches the schema in
      `src/bundle.rs` (schema constants and field names).
- [ ] `docs/pv-agent/ANCHORING.md` reflects the real endpoint contract.
- [ ] The version string in `Cargo.toml` matches what is announced to
      the client.

## F. Packaging

- [ ] `target/release/pv-agent --version` returns the expected version.
- [ ] `target/release/pv-agent --help` lists all 9 commands.
- [ ] Binary launches from a clean shell with no env vars set, and
      `pv-agent demo --plant palermo-1mw --base-dir /tmp/pv-demo`
      produces a valid bundle.
- [ ] `pv-agent.service.example` and `pv-agent.timer.example` are
      marked clearly as examples and NOT auto-enabled.

## G. Live anchoring (only when authorized)

> Do not enable this section without an explicit go from the operator
> and the IPPANCENT endpoint owner.

- [ ] Staging endpoint URL + admin token are configured locally.
- [ ] `ippan.submit_anchors = true` is set in the production config.
- [ ] A canary submission has been performed against staging and the
      L1 anchor reference is recorded in the release notes.
- [ ] `pv-agent verify --bundle <canary>` reports
      `anchor_response_matches: true`.

## H. Handover

- [ ] Operator key has been provisioned and is NOT a demo key
      (`is_demo: false` in the key file) — unless the deal explicitly
      includes a demo phase.
- [ ] `agent.production_mode = true` in the delivered config.
- [ ] Runbook (this file + `CLIENT_DELIVERY.md`) has been read by the
      operator who will run the agent.
- [ ] Contact channel exists for the client to report verification
      failures.

---

If any item in **A**, **C**, or **D** is unchecked: do not ship.

If any item in **B** is unchecked: do not ship.

If items in **E**, **F**, or **G** are unchecked but A–D pass: ship a
preview build, not a production release.
