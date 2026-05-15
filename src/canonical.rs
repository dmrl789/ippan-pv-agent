//! Canonical JSON encoder used on the hashing path.
//!
//! Rules:
//! - Object keys are sorted lexicographically (byte order on the UTF-8 key).
//! - No insignificant whitespace.
//! - Strings are JSON-escaped (subset: ", \, control chars).
//! - Numbers must be integers; floats are rejected.
//! - Output is UTF-8.
//!
//! Same logical input → same canonical bytes → same SHA-256 hash.

use crate::{Error, Result};
use serde_json::Value;
use std::io::Write;

/// Serialize a `serde_json::Value` into canonical bytes.
///
/// Returns an error if the value contains a floating-point number anywhere
/// inside the tree — floats are not allowed on the hashing path.
pub fn to_canonical_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(256);
    write_value(&mut out, value)?;
    Ok(out)
}

fn write_value(out: &mut Vec<u8>, value: &Value) -> Result<()> {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => {
            if n.is_f64() {
                return Err(Error::InvalidCanonical(format!(
                    "float not allowed in canonical payload: {}",
                    n
                )));
            }
            write!(out, "{}", n).map_err(Error::IoBare)?;
        }
        Value::String(s) => write_string(out, s),
        Value::Array(arr) => {
            out.push(b'[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_value(out, item)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_unstable();
            out.push(b'{');
            for (i, k) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_string(out, k);
                out.push(b':');
                write_value(out, map.get(*k).expect("key from map"))?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    out.push(b'"');
    for ch in s.chars() {
        match ch {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{09}' => out.extend_from_slice(b"\\t"),
            '\u{0A}' => out.extend_from_slice(b"\\n"),
            '\u{0C}' => out.extend_from_slice(b"\\f"),
            '\u{0D}' => out.extend_from_slice(b"\\r"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => {
                let mut buf = [0u8; 4];
                let bytes = c.encode_utf8(&mut buf);
                out.extend_from_slice(bytes.as_bytes());
            }
        }
    }
    out.push(b'"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sorts_object_keys() {
        let v = json!({"b": 1, "a": 2, "c": 3});
        let bytes = to_canonical_bytes(&v).unwrap();
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), r#"{"a":2,"b":1,"c":3}"#);
    }

    #[test]
    fn preserves_array_order() {
        let v = json!([3, 1, 2]);
        let bytes = to_canonical_bytes(&v).unwrap();
        assert_eq!(std::str::from_utf8(&bytes).unwrap(), "[3,1,2]");
    }

    #[test]
    fn nested_objects_sort_recursively() {
        let v = json!({"z": {"b": 1, "a": 2}, "a": {"y": 3, "x": 4}});
        let bytes = to_canonical_bytes(&v).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            r#"{"a":{"x":4,"y":3},"z":{"a":2,"b":1}}"#
        );
    }

    #[test]
    fn rejects_floats() {
        let v: Value = serde_json::from_str("{\"x\":1.5}").unwrap();
        assert!(to_canonical_bytes(&v).is_err());
    }

    #[test]
    fn escapes_strings() {
        let v = json!({"k": "a\"b\\c\n"});
        let bytes = to_canonical_bytes(&v).unwrap();
        assert_eq!(
            std::str::from_utf8(&bytes).unwrap(),
            r#"{"k":"a\"b\\c\n"}"#
        );
    }

    #[test]
    fn deterministic_repeated_calls() {
        let v = json!({"b": [{"y": 1, "x": 2}], "a": 3});
        let a = to_canonical_bytes(&v).unwrap();
        let b = to_canonical_bytes(&v).unwrap();
        assert_eq!(a, b);
    }
}
