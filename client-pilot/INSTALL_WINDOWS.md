# Install pv-agent on Windows

Tested on Windows 10 and Windows 11, x86_64. Estimated time: 10–15
minutes on a fresh machine, mostly waiting for Rust to install.

> **Client safety statement.** During the first pilot, pv-agent runs
> locally only. It does not submit data or anchors to IPPAN unless
> anchoring is explicitly enabled by configuration and command-line
> flag.

## 1. Install Rust

Open a normal **PowerShell** window (no admin needed for the default
install) and run:

```powershell
Invoke-WebRequest -Uri https://win.rustup.rs -OutFile rustup-init.exe
.\rustup-init.exe
```

When the installer prompts you, accept the default option (1). It will
download `rustc`, `cargo`, and the MSVC toolchain. After it finishes,
close PowerShell and open a fresh PowerShell window so `cargo` is on
your `PATH`.

Verify:

```powershell
cargo --version
rustc --version
```

You should see Rust 1.74 or newer.

## 2. Get the pv-agent repository

If the repo was delivered to you as a zip or folder, extract it to a
location with no spaces in the path, for example:

```
C:\IPPAN\ippan-pv-agent\
```

If it was delivered via git, clone it to the same location. (Both work
equally well; pick whichever your team uses.)

## 3. Build the release binary

```powershell
cd C:\IPPAN\ippan-pv-agent
cargo build --release
```

The first build downloads and compiles dependencies; expect 3–8 minutes
depending on the machine. Subsequent builds take a few seconds.

The binary will be at:

```
C:\IPPAN\ippan-pv-agent\target\release\pv-agent.exe
```

## 4. Run the Palermo 1MW demo

```powershell
.\target\release\pv-agent.exe demo --plant palermo-1mw
```

Expected last lines:

```
Canonical record created: YES
Signature created: YES
Evidence bundle saved: YES
IPPAN L1 anchor submitted: NO
Reason: submit_anchors=false

Bundle:
data\pv-agent\palermo-pv-001\records\2026\05\15\pv-palermo-pv-001-20260515T101500Z
```

## 5. Verify the bundle

```powershell
.\target\release\pv-agent.exe verify --bundle data\pv-agent\palermo-pv-001\records\2026\05\15\pv-palermo-pv-001-20260515T101500Z
```

Expected first line:

```
PV evidence verification: PASS
```

Expected canonical hash (must match exactly for the bundled demo):

```
sha256:ed47bc9df77ad56dc0b11f05d365b1a79adaec1f20563bcfa5b37496ca236256
```

You can re-check the hash with the built-in PowerShell tool:

```powershell
Get-FileHash data\pv-agent\palermo-pv-001\records\2026\05\15\pv-palermo-pv-001-20260515T101500Z\canonical-record.json -Algorithm SHA256
```

The lowercase hex from this command should equal the canonical hash
above (PowerShell prints uppercase — case is not significant for
hex).

## 6. Where files are stored

By default, all evidence bundles go under:

```
C:\IPPAN\ippan-pv-agent\data\pv-agent\
```

Inside that, the layout is:

```
data\pv-agent\
└── <plant_id>\
    └── records\<YYYY>\<MM>\<DD>\<record_id>\
        ├── manifest.json
        ├── canonical-record.json
        ├── signature-envelope.json
        ├── source-metadata.json
        ├── events.json
        ├── anchor-request.json
        ├── anchor-response.json
        └── verification-report.json
```

For a real deployment you would normally change the storage path to a
durable disk, e.g. `D:\IPPAN\pv-agent-data\`, via the `storage.base_dir`
setting in your `pv-agent.toml`. For the local pilot the default is
fine.

## 7. Optional next steps

- `pv-agent run-once --input <reading.json> --events <events.json>
  --config <pv-agent.toml>` — produce a real bundle from your simulator
  output. See [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md) steps 7–9.
- For staging anchoring, see [PILOT_CHECKLIST.md](PILOT_CHECKLIST.md)
  steps 10–12. **Do not enable staging anchoring without prior
  authorisation.**

## What this guide deliberately does NOT include

- Any real endpoint URL.
- Any operator key file. (Demo keys are auto-generated locally.)
- Any admin token or environment-variable value.
- Any automatic scheduling. (A scheduler is provided; it is NOT
  auto-enabled during the pilot.)
