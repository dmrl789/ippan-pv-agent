//! IPPAN / IPPANCENT L1 anchoring client.
//!
//! Sends ONLY the canonical commitment hash to L1. Full telemetry never
//! leaves the local bundle.
//!
//! Bearer tokens come from a configured environment variable and are NEVER
//! logged or written into bundles.

use crate::bundle::{read_anchor_request, write_anchor_response, AnchorRequest};
use crate::config::IppanConfig;
use crate::{Error, Result};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

const HTTP_TIMEOUT_SECS: u64 = 30;

#[derive(Debug)]
pub struct SubmitResult {
    pub response_value: Value,
}

/// Submit an anchor request from `bundle_dir` to the configured endpoint.
/// On success, writes the response to `anchor-response.json`.
///
/// Refuses to submit if:
///   - `submit_anchors=false` and `allow_submit=false`;
///   - admin token env var is not set when one is configured;
///   - the existing anchor-response is not `pending` and `force=false`.
pub fn submit(
    bundle_dir: &Path,
    ippan: &IppanConfig,
    allow_submit: bool,
    force: bool,
) -> Result<SubmitResult> {
    if !ippan.submit_anchors && !allow_submit {
        return Err(Error::Anchor(
            "submit_anchors=false in config and --submit-anchor not passed".into(),
        ));
    }

    // Refuse to overwrite a real (non-pending) response.
    let existing = crate::bundle::read_anchor_response_raw(bundle_dir)?;
    let is_pending = existing
        .get("status")
        .and_then(|s| s.as_str())
        .map(|s| s == "pending")
        .unwrap_or(false);
    if !is_pending && !force {
        return Err(Error::Anchor(
            "bundle already has a non-pending anchor response (use --force to override)".into(),
        ));
    }

    let req: AnchorRequest = read_anchor_request(bundle_dir)?;
    let req_value = serde_json::to_value(&req)?;

    let token = if let Some(env_name) = &ippan.admin_token_env {
        match std::env::var(env_name) {
            Ok(v) if !v.is_empty() => Some(v),
            _ => {
                return Err(Error::Anchor(format!(
                    "required admin token env var `{}` is not set or is empty",
                    env_name
                )));
            }
        }
    } else {
        None
    };

    let url = format!(
        "{}{}",
        ippan.endpoint.trim_end_matches('/'),
        if ippan.anchor_path.starts_with('/') {
            ippan.anchor_path.clone()
        } else {
            format!("/{}", ippan.anchor_path)
        }
    );

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build();
    let mut http_req = agent.post(&url).set("content-type", "application/json");
    if let Some(t) = &token {
        http_req = http_req.set("authorization", &format!("Bearer {}", t));
    }

    let response_value = match http_req.send_json(req_value) {
        Ok(resp) => resp
            .into_json::<Value>()
            .map_err(|e| Error::Anchor(format!("invalid JSON response: {}", e)))?,
        Err(ureq::Error::Status(code, resp)) => {
            let body = resp
                .into_string()
                .unwrap_or_else(|_| "<unreadable body>".into());
            return Err(Error::Anchor(format!(
                "anchor endpoint returned HTTP {}: {}",
                code, body
            )));
        }
        Err(e) => {
            // Note: error message must not include the bearer token. ureq's
            // transport errors do not include request headers.
            return Err(Error::Anchor(format!("transport error: {}", e)));
        }
    };

    write_anchor_response(bundle_dir, &response_value)?;
    Ok(SubmitResult { response_value })
}

/// Get current anchor status from the L1 endpoint, when supported.
/// This is a best-effort GET against `{endpoint}{anchor_path}/{reference}`.
pub fn status(bundle_dir: &Path, ippan: &IppanConfig) -> Result<Value> {
    let resp = crate::bundle::read_anchor_response_raw(bundle_dir)?;
    let reference = resp
        .get("reference")
        .and_then(|r| r.as_str())
        .ok_or_else(|| {
            Error::Anchor("no anchor reference in bundle (not yet submitted?)".into())
        })?;

    let token = if let Some(env_name) = &ippan.admin_token_env {
        std::env::var(env_name).ok().filter(|v| !v.is_empty())
    } else {
        None
    };

    let url = format!(
        "{}{}/{}",
        ippan.endpoint.trim_end_matches('/'),
        if ippan.anchor_path.starts_with('/') {
            ippan.anchor_path.clone()
        } else {
            format!("/{}", ippan.anchor_path)
        },
        reference
    );
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
        .build();
    let mut http_req = agent.get(&url);
    if let Some(t) = &token {
        http_req = http_req.set("authorization", &format!("Bearer {}", t));
    }
    match http_req.call() {
        Ok(r) => r
            .into_json::<Value>()
            .map_err(|e| Error::Anchor(format!("invalid JSON: {}", e))),
        Err(ureq::Error::Status(code, r)) => {
            let body = r.into_string().unwrap_or_default();
            Err(Error::Anchor(format!("HTTP {}: {}", code, body)))
        }
        Err(e) => Err(Error::Anchor(format!("transport error: {}", e))),
    }
}
