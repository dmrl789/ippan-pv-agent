# Install pv-agent on Linux

Tested on Ubuntu 22.04+ and Debian 12, x86_64 and arm64. Estimated time:
10–15 minutes on a fresh machine, mostly waiting for Rust to install.

> **Client safety statement.** During the first pilot, pv-agent runs
> locally only. It does not submit data or anchors to IPPAN unless
> anchoring is explicitly enabled by configuration and command-line
> flag.

## 1. Install Rust

One line, no sudo required:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Accept the default option (1) when prompted. After the installer
finishes, either open a fresh shell or run:

```bash
source "$HOME/.cargo/env"
```

Verify:

```bash
cargo --version
rustc --version
```

You should see Rust 1.74 or newer.

## 2. Get the pv-agent repository

If the repo was delivered as a tarball or folder, extract it somewhere
under your home directory or `/opt`:

```bash
sudo mkdir -p /opt/ippan
sudo chown -R "$USER:$USER" /opt/ippan
cd /opt/ippan
# extract the delivered archive here, OR git clone if you use git
```

For the pilot, no system-level install is required; building inside a
user-writable directory is enough.

## 3. Build the release binary

```bash
cd /opt/ippan/ippan-pv-agent
cargo build --release
```

The first build downloads and compiles dependencies; expect 3–8 minutes
depending on the machine.

The binary will be at:

```
/opt/ippan/ippan-pv-agent/target/release/pv-agent
```

## 4. Run the Palermo 1MW demo

```bash
./target/release/pv-agent demo --plant palermo-1mw
```

Expected last lines:

```
Canonical record created: YES
Signature created: YES
Evidence bundle saved: YES
IPPAN L1 anchor submitted: NO
Reason: submit_anchors=false

Bundle:
data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

## 5. Verify the bundle

```bash
./target/release/pv-agent verify --bundle data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z
```

Expected first line:

```
PV evidence verification: PASS
```

Expected canonical hash (must match exactly for the bundled demo):

```
sha256:c2150ae864a62d1bbdc284b81a55494656fff3b218838bab8fc848efa94d9171
```

You can re-check the hash with the standard tool:

```bash
sha256sum data/pv-agent/palermo-pv-001/records/2026/05/15/pv-palermo-pv-001-20260515T101500Z/canonical-record.json
# → c2150ae864a62d1bbdc284b81a55494656fff3b218838bab8fc848efa94d9171
```

## 6. Where files are stored

```
/opt/ippan/ippan-pv-agent/data/pv-agent/
└── <plant_id>/
    └── records/<YYYY>/<MM>/<DD>/<record_id>/
        ├── manifest.json
        ├── canonical-record.json
        ├── signature-envelope.json
        ├── source-metadata.json
        ├── events.json
        ├── anchor-request.json
        ├── anchor-response.json
        └── verification-report.json
```

For a real deployment, change `storage.base_dir` in your `pv-agent.toml`
to a durable, backed-up path (e.g. `/var/lib/pv-agent/`).

## 7. Optional: install as a system binary

```bash
sudo install -m 0755 target/release/pv-agent /usr/local/bin/pv-agent
pv-agent --help
```

After this, you can drop `./target/release/` from all commands.

## 8. Optional: systemd timer (NOT enabled by default)

The repository ships two example unit files:

```
docs/pv-agent/pv-agent.service.example
docs/pv-agent/pv-agent.timer.example
```

These are **examples**. For the pilot, do **not** enable them. If, after
the pilot, you decide to run pv-agent on a recurring schedule, copy
them as a starting point:

```bash
sudo cp docs/pv-agent/pv-agent.service.example /etc/systemd/system/pv-agent.service
sudo cp docs/pv-agent/pv-agent.timer.example   /etc/systemd/system/pv-agent.timer
sudo $EDITOR /etc/systemd/system/pv-agent.service   # adjust paths
# Only then:
# sudo systemctl daemon-reload
# sudo systemctl enable --now pv-agent.timer
```

The pilot deliberately does NOT touch systemd. A run is one CLI
invocation.

## 9. Optional next steps

- `pv-agent run-once` against a real reading from your simulator
  (see [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md) steps 7–9).
- For staging anchoring, see [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md)
  steps 10–12. **Do not enable staging anchoring without prior
  authorisation.**

## What this guide deliberately does NOT include

- Any real endpoint URL.
- Any operator key file. (Demo keys are auto-generated locally.)
- Any admin token or environment-variable value.
- Any automatic scheduling.
