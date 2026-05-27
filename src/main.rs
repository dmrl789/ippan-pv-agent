//! pv-agent CLI entry point.

use clap::{Parser, Subcommand};
use ippan_pv_agent::anchor;
use ippan_pv_agent::bundle::{build_bundle, BuildOptions};
use ippan_pv_agent::config::{
    AgentConfig, Config, EventsConfig, IppanConfig, KeyConfig, StorageConfig,
};
use ippan_pv_agent::demo::{palermo_events, palermo_raw_input};
use ippan_pv_agent::events::Event;
use ippan_pv_agent::inspect::inspect;
use ippan_pv_agent::signing::OperatorKey;
use ippan_pv_agent::telemetry::RawInput;
use ippan_pv_agent::verify::verify_local;
use ippan_pv_agent::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "pv-agent",
    version,
    about = "IPPAN photovoltaic plant agent — deterministic evidence, signed, L1-anchored"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Build a deterministic demo evidence bundle (Palermo 1MW).
    Demo {
        #[arg(long, default_value = "palermo-1mw")]
        plant: String,
        /// Override storage base dir for the demo (defaults to ./data/pv-agent).
        #[arg(long)]
        base_dir: Option<PathBuf>,
        /// Override the operator key file path (defaults to <base_dir>/keys/demo-key.json).
        #[arg(long)]
        key_file: Option<PathBuf>,
        /// Submit the resulting commitment to the IPPAN endpoint. Requires
        /// configured endpoint + admin token.
        #[arg(long)]
        submit_anchor: bool,
        /// IPPAN endpoint override for one-shot submission.
        #[arg(long)]
        endpoint: Option<String>,
        /// Allow overwriting an existing bundle.
        #[arg(long)]
        force: bool,
    },

    /// Build one evidence bundle from input files.
    RunOnce {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        events: Option<PathBuf>,
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        key_file: Option<PathBuf>,
        /// Allow the use of a demo key in a production-mode config.
        #[arg(long)]
        allow_demo_key: bool,
        #[arg(long)]
        force: bool,
    },

    /// Verify a local evidence bundle.
    Verify {
        #[arg(long)]
        bundle: PathBuf,
    },

    /// Print a human-readable bundle summary (no secrets).
    Inspect {
        #[arg(long)]
        bundle: PathBuf,
    },

    /// Submit a bundle commitment to IPPAN L1.
    AnchorSubmit {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        config: PathBuf,
        /// Force submission even if submit_anchors=false in config.
        #[arg(long = "submit-anchor")]
        submit_anchor: bool,
        /// Re-submit an already-anchored bundle.
        #[arg(long)]
        force: bool,
    },

    /// Retrieve current L1 anchor status.
    AnchorStatus {
        #[arg(long)]
        bundle: PathBuf,
        #[arg(long)]
        config: PathBuf,
    },

    /// Write a default config file.
    Init {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "palermo-pv-001")]
        plant_id: String,
    },

    /// Generate a demo Ed25519 operator key.
    GenerateDemoKey {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "key:demo-local")]
        key_ref: String,
    },

    /// Run the scheduler loop (every 15 minutes by default).
    Schedule {
        #[arg(long)]
        config: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        events: Option<PathBuf>,
        #[arg(long)]
        key_file: Option<PathBuf>,
        /// Interval in minutes (default 15).
        #[arg(long, default_value_t = 15u64)]
        interval_minutes: u64,
        /// Run only N iterations then exit (0 = forever).
        #[arg(long, default_value_t = 0u64)]
        max_iterations: u64,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.cmd {
        Cmd::Demo {
            plant,
            base_dir,
            key_file,
            submit_anchor,
            endpoint,
            force,
        } => cmd_demo(&plant, base_dir, key_file, submit_anchor, endpoint, force),
        Cmd::RunOnce {
            input,
            events,
            config,
            key_file,
            allow_demo_key,
            force,
        } => cmd_run_once(
            &input,
            events.as_deref(),
            &config,
            key_file.as_deref(),
            allow_demo_key,
            force,
        ),
        Cmd::Verify { bundle } => cmd_verify(&bundle),
        Cmd::Inspect { bundle } => cmd_inspect(&bundle),
        Cmd::AnchorSubmit {
            bundle,
            config,
            submit_anchor,
            force,
        } => cmd_anchor_submit(&bundle, &config, submit_anchor, force),
        Cmd::AnchorStatus { bundle, config } => cmd_anchor_status(&bundle, &config),
        Cmd::Init { out, plant_id } => cmd_init(&out, &plant_id),
        Cmd::GenerateDemoKey { out, key_ref } => cmd_generate_demo_key(&out, &key_ref),
        Cmd::Schedule {
            config,
            input,
            events,
            key_file,
            interval_minutes,
            max_iterations,
        } => cmd_schedule(
            &config,
            &input,
            events.as_deref(),
            key_file.as_deref(),
            interval_minutes,
            max_iterations,
        ),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {}", e);
            ExitCode::from(1)
        }
    }
}

fn cmd_demo(
    plant: &str,
    base_dir: Option<PathBuf>,
    key_file: Option<PathBuf>,
    submit_anchor: bool,
    endpoint: Option<String>,
    force: bool,
) -> Result<()> {
    if plant != "palermo-1mw" {
        return Err(Error::Other(format!(
            "unknown demo plant `{}` (only `palermo-1mw` is bundled)",
            plant
        )));
    }
    let base = base_dir.unwrap_or_else(|| PathBuf::from("data/pv-agent"));
    fs::create_dir_all(&base).map_err(|e| Error::io(&base, e))?;

    let key_path = key_file
        .clone()
        .unwrap_or_else(|| base.join("keys/demo-key.json"));
    let key = if key_path.exists() {
        OperatorKey::load_from_file(&key_path)?
    } else {
        let k = OperatorKey::generate_demo("key:plant-palermo-001");
        k.save_to_file(&key_path)?;
        k
    };

    let cfg = Config {
        agent: AgentConfig {
            agent_id: "pv-agent-palermo-001".into(),
            agent_type: "pv_plant_agent".into(),
            plant_id: "palermo-pv-001".into(),
            operator_key_ref: key.key_ref.clone(),
            production_mode: false,
        },
        storage: StorageConfig {
            base_dir: base.to_string_lossy().into_owned(),
        },
        ippan: IppanConfig {
            endpoint: endpoint.unwrap_or_else(|| "http://127.0.0.1:18181".into()),
            anchor_path: "/v1/anchors".into(),
            admin_token_env: Some("IPPAN_ADMIN_TOKEN".into()),
            submit_anchors: false,
        },
        events: EventsConfig {
            lookback_minutes: 240,
        },
        key: KeyConfig {
            key_file: Some(key_path.to_string_lossy().into_owned()),
        },
    };

    let raw = palermo_raw_input();
    let events = palermo_events();

    let opts = BuildOptions {
        force,
        interval_window_minutes: 15,
    };
    let built = build_bundle(&cfg, &raw, &events, &key, &opts)?;

    println!("PV Agent Demo — Palermo 1MW");
    println!();
    println!("Telemetry interval: {} minutes", raw.interval_minutes);
    println!("GHI: {} W/m²", raw.telemetry.ghi_w_m2);
    println!("Temperature: {} °C", raw.telemetry.ambient_temperature_c);
    println!("DC power: {} kW", raw.telemetry.dc_power_kw);
    println!("AC power: {} kW", raw.telemetry.ac_power_kw);
    println!("Meter power: {} kW", raw.telemetry.meter_power_kw);
    println!("Performance ratio: {}", raw.telemetry.performance_ratio);
    println!(
        "Energy since start: {} kWh",
        raw.telemetry.energy_since_start_kwh
    );
    println!();
    println!("Canonical record created: YES");
    println!("Signature created: YES");
    println!("Evidence bundle saved: YES");

    let mut submitted = false;
    if submit_anchor {
        match anchor::submit(&built.bundle_dir, &cfg.ippan, true, force) {
            Ok(_) => {
                println!("IPPAN L1 anchor submitted: YES");
                submitted = true;
            }
            Err(e) => {
                println!("IPPAN L1 anchor submitted: NO");
                println!("Reason: {}", e);
            }
        }
    }
    if !submitted && !submit_anchor {
        println!("IPPAN L1 anchor submitted: NO");
        println!("Reason: submit_anchors=false");
    }

    println!();
    println!("Bundle:");
    println!("{}", built.bundle_dir.display());

    Ok(())
}

fn cmd_run_once(
    input: &Path,
    events_path: Option<&Path>,
    config_path: &Path,
    key_file_override: Option<&Path>,
    allow_demo_key: bool,
    force: bool,
) -> Result<()> {
    let cfg = Config::load(config_path)?;

    let raw_bytes = fs::read(input).map_err(|e| Error::io(input, e))?;
    let raw: RawInput = serde_json::from_slice(&raw_bytes)?;

    let events: Vec<Event> = if let Some(p) = events_path {
        let bytes = fs::read(p).map_err(|e| Error::io(p, e))?;
        serde_json::from_slice(&bytes)?
    } else {
        vec![]
    };

    let key_path: PathBuf = match key_file_override {
        Some(p) => p.to_path_buf(),
        None => match cfg.key.key_file.clone() {
            Some(s) => PathBuf::from(s),
            None => PathBuf::from(&cfg.storage.base_dir).join("keys/demo-key.json"),
        },
    };
    if !key_path.exists() {
        return Err(Error::Config(format!(
            "operator key file not found at {} — generate with `pv-agent generate-demo-key`",
            key_path.display()
        )));
    }
    let key = OperatorKey::load_from_file(&key_path)?;
    if cfg.agent.production_mode && key.is_demo && !allow_demo_key {
        return Err(Error::DemoKeyInProduction);
    }

    let opts = BuildOptions {
        force,
        interval_window_minutes: raw.interval_minutes as i64,
    };
    let built = build_bundle(&cfg, &raw, &events, &key, &opts)?;
    println!("Record ID:        {}", built.record_id);
    println!("Canonical hash:   {}", built.canonical_hash);
    println!("Bundle:           {}", built.bundle_dir.display());
    Ok(())
}

fn cmd_verify(bundle: &Path) -> Result<()> {
    let report = verify_local(bundle)?;
    if report.overall_pass {
        println!("PV evidence verification: PASS");
    } else {
        println!("PV evidence verification: FAIL");
    }
    println!("record_id: {}", report.record_id);
    println!("plant_id: {}", report.plant_id);
    println!("canonical_hash: {}", report.canonical_hash);
    if let Some(reference) = &report.anchor_reference {
        println!("l1_reference: {}", reference);
    }
    println!("checks:");
    println!(
        "  canonical_reproducible:           {}",
        report.canonical_reproducible
    );
    println!(
        "  canonical_hash_matches_manifest:  {}",
        report.canonical_hash_matches_manifest
    );
    println!(
        "  signature_valid:                  {}",
        report.signature_valid
    );
    println!(
        "  manifest_files_intact:            {}",
        report.manifest_files_intact
    );
    println!(
        "  anchor_request_matches:           {}",
        report.anchor_request_matches
    );
    match report.anchor_response_matches {
        Some(true) => {
            println!("  anchor_response_matches:          true (L1 anchor verification: PASS)")
        }
        Some(false) => {
            println!("  anchor_response_matches:          false (L1 anchor verification: FAIL)")
        }
        None => println!("  anchor_response_matches:          n/a (pending)"),
    }
    if !report.overall_pass {
        return Err(Error::Verification(format!(
            "bundle {} did not pass verification",
            bundle.display()
        )));
    }
    Ok(())
}

fn cmd_inspect(bundle: &Path) -> Result<()> {
    let r = inspect(bundle)?;
    for line in r.lines {
        println!("{}", line);
    }
    Ok(())
}

fn cmd_anchor_submit(
    bundle: &Path,
    config_path: &Path,
    submit_anchor: bool,
    force: bool,
) -> Result<()> {
    let cfg = Config::load(config_path)?;
    // Refuse to submit if bundle does not verify.
    let report = verify_local(bundle)?;
    if !report.overall_pass {
        return Err(Error::Anchor(
            "refusing to submit: local verification failed".into(),
        ));
    }
    let res = anchor::submit(bundle, &cfg.ippan, submit_anchor, force)?;
    println!("Anchor submitted: YES");
    if let Some(s) = res.response_value.get("status").and_then(|v| v.as_str()) {
        println!("Anchor status: {}", s);
    }
    if let Some(r) = res.response_value.get("reference").and_then(|v| v.as_str()) {
        println!("Anchor reference: {}", r);
    }
    Ok(())
}

fn cmd_anchor_status(bundle: &Path, config_path: &Path) -> Result<()> {
    let cfg = Config::load(config_path)?;
    let v = anchor::status(bundle, &cfg.ippan)?;
    println!("{}", serde_json::to_string_pretty(&v)?);
    Ok(())
}

fn cmd_init(out: &Path, plant_id: &str) -> Result<()> {
    if out.exists() {
        return Err(Error::Config(format!(
            "refusing to overwrite existing file: {}",
            out.display()
        )));
    }
    let agent_id = format!("pv-agent-{}", plant_id);
    let key_ref = format!("key:plant-{}", plant_id);
    let body = format!(
        r#"# pv-agent configuration

[agent]
agent_id = "{agent_id}"
agent_type = "pv_plant_agent"
plant_id = "{plant_id}"
operator_key_ref = "{key_ref}"
production_mode = false

[storage]
base_dir = "data/pv-agent"

[ippan]
endpoint = "http://127.0.0.1:18181"
anchor_path = "/v1/anchors"
admin_token_env = "IPPAN_ADMIN_TOKEN"
submit_anchors = false

[events]
lookback_minutes = 240

[key]
# Path to the Ed25519 operator key JSON file. Use `pv-agent generate-demo-key`
# to create a demo key, or import a production key produced by your operator
# key tooling.
key_file = "data/pv-agent/keys/operator-key.json"
"#,
        agent_id = agent_id,
        plant_id = plant_id,
        key_ref = key_ref,
    );
    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
    }
    fs::write(out, body).map_err(|e| Error::io(out, e))?;
    println!("Wrote {}", out.display());
    Ok(())
}

fn cmd_generate_demo_key(out: &Path, key_ref: &str) -> Result<()> {
    if out.exists() {
        return Err(Error::Config(format!(
            "refusing to overwrite existing key file: {}",
            out.display()
        )));
    }
    let k = OperatorKey::generate_demo(key_ref);
    k.save_to_file(out)?;
    println!("Generated demo Ed25519 key.");
    println!("Key ref: {}", key_ref);
    println!("File:    {}", out.display());
    println!();
    println!("WARNING: this is a DEMO KEY. Production mode will refuse to use it");
    println!("unless --allow-demo-key is explicitly passed.");
    Ok(())
}

fn cmd_schedule(
    config_path: &Path,
    input: &Path,
    events_path: Option<&Path>,
    key_file_override: Option<&Path>,
    interval_minutes: u64,
    max_iterations: u64,
) -> Result<()> {
    let mut iter: u64 = 0;
    loop {
        iter += 1;
        println!("[schedule] iteration {} starting", iter);
        match cmd_run_once(
            input,
            events_path,
            config_path,
            key_file_override,
            false,
            false,
        ) {
            Ok(()) => {}
            Err(e) => eprintln!("[schedule] iteration {} failed: {}", iter, e),
        }
        if max_iterations != 0 && iter >= max_iterations {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(interval_minutes * 60));
    }
}
