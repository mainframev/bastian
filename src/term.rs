//! Terminal queries. These talk to `/dev/tty` directly, never stdin or
//! stdout, so they work when output is piped or redirected.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Dark,
    Light,
}

/// How long to wait for the terminal to answer before giving up.
const QUERY_TIMEOUT: Duration = Duration::from_millis(100);

/// The terminal's background brightness: asked with OSC 11, else guessed
/// from `COLORFGBG`. `None` when neither says.
pub fn background() -> Option<Background> {
    query_background(QUERY_TIMEOUT)
        .or_else(|| from_colorfgbg(std::env::var("COLORFGBG").ok().as_deref()?))
}

/// Parse an OSC 11 reply such as `\e]11;rgb:1919/1717/2424\e\\`.
pub fn parse_osc11(reply: &str) -> Option<Background> {
    let rgb = reply.split("rgb:").nth(1)?;
    let channels: Vec<f32> = rgb
        .split('/')
        .take(3)
        .map(|c| {
            let hex: String = c.chars().take_while(char::is_ascii_hexdigit).collect();
            let max = 16f32.powi(hex.len() as i32) - 1.0;
            u32::from_str_radix(&hex, 16).ok().map(|v| v as f32 / max)
        })
        .collect::<Option<_>>()?;
    let [r, g, b] = channels[..] else { return None };
    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    Some(if luminance > 0.5 {
        Background::Light
    } else {
        Background::Dark
    })
}

/// `COLORFGBG` is `fg;bg` (or `fg;default;bg`) with ANSI color indices; set by
/// rxvt, Konsole and some others. Backgrounds 7 and 9–15 are light.
pub fn from_colorfgbg(value: &str) -> Option<Background> {
    let bg: u8 = value.rsplit(';').next()?.trim().parse().ok()?;
    Some(if bg == 7 || (9..=15).contains(&bg) {
        Background::Light
    } else {
        Background::Dark
    })
}

#[cfg(unix)]
fn query_background(timeout: Duration) -> Option<Background> {
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::fd::AsRawFd;

    let mut tty = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .ok()?;
    let fd = tty.as_raw_fd();

    // Raw mode so the reply isn't echoed or line-buffered; always restored.
    // SAFETY: termios calls on a file descriptor we own, with a zeroed struct
    // that tcgetattr fills before use.
    let mut saved: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &raw mut saved) } != 0 {
        return None;
    }
    let mut raw_mode = saved;
    unsafe { libc::cfmakeraw(&raw mut raw_mode) };
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw const raw_mode) } != 0 {
        return None;
    }

    let mut reply = Vec::new();
    if tty.write_all(b"\x1b]11;?\x1b\\").is_ok() && tty.flush().is_ok() {
        let deadline = std::time::Instant::now() + timeout;
        let mut buf = [0u8; 64];
        // Read until BEL or ST ends the reply, or time runs out.
        while !reply.ends_with(b"\x07") && !reply.ends_with(b"\x1b\\") {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            if left.is_zero() || !readable(fd, left) {
                break;
            }
            match tty.read(&mut buf) {
                Ok(n) if n > 0 => reply.extend_from_slice(&buf[..n]),
                _ => break,
            }
        }
    }
    unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw const saved) };
    parse_osc11(&String::from_utf8_lossy(&reply))
}

/// Wait up to `timeout` for input on `fd`. Uses select(2): on macOS,
/// poll(2) doesn't support terminal devices and reports them ready at once,
/// which would turn the following read into an unbounded block.
#[cfg(unix)]
fn readable(fd: std::os::fd::RawFd, timeout: Duration) -> bool {
    // SAFETY: an fd_set zeroed and filled with FD_ZERO/FD_SET for one
    // descriptor we own, and a timeval on the stack, as select(2) expects.
    unsafe {
        let mut set: libc::fd_set = std::mem::zeroed();
        libc::FD_ZERO(&raw mut set);
        libc::FD_SET(fd, &raw mut set);
        let mut tv = libc::timeval {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_usec: timeout.subsec_micros() as libc::suseconds_t,
        };
        libc::select(
            fd + 1,
            &raw mut set,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &raw mut tv,
        ) > 0
            && libc::FD_ISSET(fd, &raw const set)
    }
}

#[cfg(not(unix))]
fn query_background(_timeout: Duration) -> Option<Background> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_osc11_replies() {
        // Rosé Pine base (#191724) and Dawn base (#faf4ed), 4-digit channels.
        assert_eq!(
            parse_osc11("\x1b]11;rgb:1919/1717/2424\x1b\\"),
            Some(Background::Dark)
        );
        assert_eq!(
            parse_osc11("\x1b]11;rgb:fafa/f4f4/eded\x07"),
            Some(Background::Light)
        );
        // 2-digit channels.
        assert_eq!(
            parse_osc11("\x1b]11;rgb:ff/ff/ff\x07"),
            Some(Background::Light)
        );
        assert_eq!(parse_osc11("garbage"), None);
        assert_eq!(parse_osc11(""), None);
    }

    #[test]
    fn parses_colorfgbg() {
        assert_eq!(from_colorfgbg("15;0"), Some(Background::Dark));
        assert_eq!(from_colorfgbg("0;15"), Some(Background::Light));
        assert_eq!(from_colorfgbg("0;default;7"), Some(Background::Light));
        assert_eq!(from_colorfgbg("nonsense"), None);
    }
}
