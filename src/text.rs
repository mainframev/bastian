//! Text in and out of documents: reading files from disk (byte-order
//! marks and UTF-16 handled here once) and fitting strings into a width
//! of terminal cells.

use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, bail};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Largest document bastian reads from disk.
const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// Read a text file, decoding UTF-8 (with or without a BOM) and UTF-16
/// files that start with a BOM.
pub fn read_text(path: &Path) -> Result<String> {
    let bytes = read_capped(path, MAX_BYTES)?;
    decode(&bytes).with_context(|| format!("reading {}", path.display()))
}

/// Read a regular file of at most `max` bytes. Anything else (a FIFO, a
/// device such as `/dev/zero`) is an error rather than a read that blocks
/// or never ends.
pub(crate) fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>> {
    let context = || format!("reading {}", path.display());
    // Checked before opening: opening a FIFO blocks until a writer comes.
    anyhow::ensure!(
        std::fs::metadata(path).with_context(context)?.is_file(),
        "{} is not a file",
        path.display()
    );
    let file = std::fs::File::open(path).with_context(context)?;
    let mut bytes = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .with_context(context)?;
    anyhow::ensure!(bytes.len() as u64 <= max, "{} is too large", path.display());
    Ok(bytes)
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
pub(crate) fn decode_lossy(bytes: &[u8]) -> String {
    decode(bytes).unwrap_or_else(|_| String::from_utf8_lossy(bytes).into_owned())
}

/// Keep the last `max` columns of `s`, marking the cut with `…`.
pub fn shorten_left(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut keep = Vec::new();
    let mut w = 0;
    for c in s.chars().rev() {
        let cw = c.width().unwrap_or(0);
        if w + cw + 1 > max {
            break;
        }
        keep.push(c);
        w += cw;
    }
    format!("…{}", keep.iter().rev().collect::<String>())
}

/// Keep the first `max` columns of `s`, marking the cut with `…`.
pub fn shorten_right(s: &str, max: usize) -> String {
    if s.width() <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    format!("{}…", take_width(s, max - 1))
}

/// The longest start of `s` that fits in `max` columns.
pub fn take_width(s: &str, max: usize) -> &str {
    let mut w = 0;
    for (i, c) in s.char_indices() {
        w += c.width().unwrap_or(0);
        if w > max {
            return &s[..i];
        }
    }
    s
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
    let units: Vec<u16> = bytes.as_chunks().0.iter().map(|&c| unit(c)).collect();
    String::from_utf16_lossy(&units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortens_paths_from_the_left() {
        assert_eq!(shorten_left("~/code/app", 20), "~/code/app");
        assert_eq!(shorten_left("~/code/app", 6), "…e/app");
        assert_eq!(shorten_left("~/code/app", 0), "");
        // Wide characters count by the columns they take.
        assert_eq!(shorten_left("日本語/説明書.md", 10), "…説明書.md");
        assert!(shorten_left("日本語/説明書.md", 10).width() <= 10);
    }

    #[test]
    fn shortens_titles_from_the_right() {
        assert_eq!(shorten_right("Guide", 10), "Guide");
        assert_eq!(shorten_right("A long title", 6), "A lon…");
        assert_eq!(shorten_right("Guide", 0), "");
    }

    #[test]
    fn takes_whole_characters_within_the_width() {
        assert_eq!(take_width("abc", 2), "ab");
        assert_eq!(take_width("日本語", 5), "日本");
        assert_eq!(take_width("日本語", 1), "");
        assert_eq!(take_width("ab", 9), "ab");
    }

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
    fn reads_only_regular_files_up_to_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.md");
        std::fs::write(&file, "12345").unwrap();
        assert_eq!(read_capped(&file, 5).unwrap(), b"12345");
        let err = read_capped(&file, 4).unwrap_err().to_string();
        assert!(err.ends_with("is too large"), "{err}");
        let err = read_capped(dir.path(), 5).unwrap_err().to_string();
        assert!(err.ends_with("is not a file"), "{err}");
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
