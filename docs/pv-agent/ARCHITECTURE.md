# Architecture

```
ippan-pv-agent/
├── src/
│   ├── main.rs           # CLI entry (clap)
│   ├── lib.rs            # library root
│   ├── config.rs         # TOML config loader
│   ├── telemetry.rs      # raw input + canonical scaled-integer telemetry
│   ├── events.rs         # event types, validation, deterministic sort, attachment rules
│   ├── canonical.rs      # canonical JSON encoder (sorted keys, integer-only)
│   ├── hashing.rs        # SHA-256 wrapper
│   ├── signing.rs        # Ed25519 signing + envelope build/verify + key file I/O
│   ├── bundle.rs         # bundle assembly + atomic writes + manifest
│   ├── anchor.rs         # IPPAN L1 HTTP anchor client (ureq)
│   ├── verify.rs         # local verification + verification-report.json
│   ├── inspect.rs        # human-readable bundle summary (no secrets)
│   └── demo.rs           # Palermo 1MW canned data
├── examples/pv/          # example telemetry, events, config
├── docs/pv-agent/        # documentation set
└── tests/                # integration tests (no live network)
```

## Module dependencies

```
   telemetry ──┐
   events  ───┤
   canonical ─┤
   hashing  ──┼─▶ bundle ─▶ verify
   signing ───┤              ▲
   config ────┘              │
   anchor    ─────────────────┘
   inspect   ──▶ bundle
   demo      ──▶ telemetry, events
```

Everything that touches the canonical hash flows through `telemetry`,
`events`, and `canonical`. Anything that does **not** touch the hash —
HTTP, logs, inspect output, source metadata — sits to the side and cannot
change the commitment.

## Why the canonical record is a `serde_json::Value` and not a typed struct

The canonical JSON encoder accepts any `Value`. We deliberately build the
canonical record as an `IndexMap`/`Map` and let the encoder sort keys at
emit time. This means:

1. The struct definition cannot drift away from the canonical bytes.
2. Adding a new field is a localized change in `bundle::canonical_record_value`.
3. The encoder rejects floats anywhere in the tree, regardless of which
   field they appear in — a single defensive choke point.

## Atomic writes

`bundle::atomic_write` writes to `<path>.tmp`, fsyncs the file, renames to
the target path, and fsyncs the parent directory where supported. This
guarantees that a verifier never observes a half-written bundle file.
