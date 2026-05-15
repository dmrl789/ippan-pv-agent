//! ippan-pv-agent: client-side PV plant agent for IPPAN.
//!
//! Produces deterministic canonical PV evidence records, signs them with
//! Ed25519, stores complete evidence bundles locally, and anchors only the
//! commitment hash to IPPAN / IPPANCENT L1.

pub mod anchor;
pub mod bundle;
pub mod canonical;
pub mod config;
pub mod demo;
pub mod errors;
pub mod events;
pub mod hashing;
pub mod inspect;
pub mod signing;
pub mod telemetry;
pub mod verify;

pub use errors::{Error, Result};
