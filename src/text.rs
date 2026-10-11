//! Reading documents from disk: byte-order marks and UTF-16 are handled
//! here once, for every place bastian opens a file.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// Read a text file, decoding UTF-8 (with or without a BOM) and UTF-16
/// files that start with a BOM.
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    decode(&bytes).with_context(|| format!("reading {}", path.display()))
}

/// Decode file bytes; see `read_text`.
pub fn decode(bytes: &[u8]) -> Result<String> {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return utf8(rest);
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return Ok(utf16(rest, u16::from_le_bytes));
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return Ok(utf16(rest, u16::from_be_bytes));
    }
    utf8(bytes)
}

/// Like `decode`, but never fails: invalid bytes become U+FFFD. For
/// best-effort reads such as a title from the start of a file.
pub fn decode_lossy(bytes: &[u8]) -> String {
    decode(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

fn utf8(bytes: &[u8]) -> Result<String> {
    match std::str::from_utf8(bytes) {
        Ok(s) => Ok(s.to_string()),
        Err(e) => bail!(
            "not valid UTF-8 (byte {}); bastian reads UTF-8 and UTF-16 with a byte-order mark",
            e.valid_up_to()
        ),
    }
}

/// UTF-16 code units, decoding lossily (an odd trailing byte is dropped).
fn utf16(bytes: &[u8], unit: fn([u8; 2]) -> u16) -> String {
    let units: Vec<u16> = bytes.chunks_exact(2).map(|c| unit([c[0], c[1]])).collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(s: &str) -> Vec<u8> {
        let mut out = vec![0xFF, 0xFE];
        for u in s.encode_utf16() {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out
    }

    #[test]
    fn decodes_boms() {
        assert_eq!(decode(b"\xEF\xBB\xBF# Hi").unwrap(), "# Hi");
        assert_eq!(decode(&utf16le("# Grüße")).unwrap(), "# Grüße");
        let be: Vec<u8> = [0xFE, 0xFF, 0x00, b'#', 0x00, b' ', 0x00, b'x'].to_vec();
        assert_eq!(decode(&be).unwrap(), "# x");
        assert_eq!(decode(b"plain").unwrap(), "plain");
    }

    #[test]
    fn invalid_utf8_is_a_clear_error() {
        let err = decode(b"ok\xFFbad").unwrap_err().to_string();
        assert!(
            err.contains("not valid UTF-8") && err.contains("byte 2"),
            "{err}"
        );
        assert_eq!(decode_lossy(b"ok\xFF"), "ok\u{FFFD}");
    }
}
