//! SHA-256 hashing wrapper used for the canonical commitment.

use sha2::{Digest, Sha256};

/// SHA-256 over `data`, returned as `"sha256:<hex>"`.
pub fn sha256_prefixed_hex(data: &[u8]) -> String {
    format!("sha256:{}", sha256_hex(data))
}

/// Bare hex of SHA-256.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Parse `"sha256:<hex>"` into raw 32 bytes (used for verification).
pub fn parse_prefixed_hex(s: &str) -> Result<[u8; 32], String> {
    let h = s
        .strip_prefix("sha256:")
        .ok_or_else(|| format!("expected sha256: prefix, got {}", s))?;
    let bytes = hex::decode(h).map_err(|e| format!("invalid hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(format!("expected 32 bytes, got {}", bytes.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vector_empty() {
        assert_eq!(
            sha256_prefixed_hex(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn known_vector_abc() {
        assert_eq!(
            sha256_prefixed_hex(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn round_trip_parse() {
        let h = sha256_prefixed_hex(b"hello");
        let bytes = parse_prefixed_hex(&h).unwrap();
        assert_eq!(hex::encode(bytes), &h["sha256:".len()..]);
    }
}
