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
fn parse_osc11(reply: &str) -> Option<Background> {
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
fn from_colorfgbg(value: &str) -> Option<Background> {
    let bg: u8 = value.rsplit(';').next()?.trim().parse().ok()?;
    Some(if bg == 7 || (9..=15).contains(&bg) {
        Background::Light
    } else {
        Background::Dark
    })
}

fn query_background(timeout: Duration) -> Option<Background> {
    let reply = ask(b"\x1b]11;?\x1b\\", timeout, |r| {
        r.ends_with(b"\x07") || r.ends_with(b"\x1b\\")
    })?;
    parse_osc11(&String::from_utf8_lossy(&reply))
}

/// What the terminal can draw pictures with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Graphics {
    /// The kitty graphics protocol (kitty, Ghostty and others).
    pub kitty: bool,
    /// Sixel, from the device attributes.
    pub sixel: bool,
    /// Pixel size of a character cell, when the terminal reports it.
    pub cell: Option<(u16, u16)>,
}

/// Ask the terminal about graphics: a kitty graphics query, the cell size
/// and the device attributes, which every terminal answers, so the wait
/// normally ends as soon as the replies are in. Run before the TUI takes
/// over the keyboard.
pub fn graphics() -> Graphics {
    // A 1×1 query image; supporting terminals reply `_Gi=31;OK`.
    const QUERY: &[u8] = b"\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\\x1b[16t\x1b[c";
    ask(QUERY, Duration::from_millis(200), |r| da1_end(r).is_some())
        .map(|reply| parse_graphics(&reply))
        .unwrap_or_default()
}

/// Read the replies to graphics queries.
fn parse_graphics(reply: &[u8]) -> Graphics {
    let text = String::from_utf8_lossy(reply);
    let kitty = text.contains("_Gi=31;OK");
    // `ESC [ ? 62 ; 4 ; … c`: attribute 4 is sixel.
    let sixel = text
        .split("\x1b[?")
        .nth(1)
        .and_then(|rest| rest.split('c').next())
        .is_some_and(|attrs| attrs.split(';').any(|a| a == "4"));
    // `ESC [ 6 ; height ; width t`.
    let cell = text.split("\x1b[6;").nth(1).and_then(|rest| {
        let (h, rest) = rest.split_once(';')?;
        let w = rest.split('t').next()?;
        let (h, w) = (h.parse().ok()?, w.parse().ok()?);
        (h > 0 && w > 0).then_some((w, h))
    });
    Graphics { kitty, sixel, cell }
}

/// End of a device-attributes reply (`ESC [ ? … c`) in `reply`, if it's in.
fn da1_end(reply: &[u8]) -> Option<usize> {
    let start = reply.windows(3).position(|w| w == b"\x1b[?")?;
    reply[start..]
        .iter()
        .position(|&b| b == b'c')
        .map(|i| start + i)
}

/// Write `query` to the terminal and read its reply until `done` says it's
/// complete or `timeout` passes. Talks to `/dev/tty` in raw mode, always
/// restored, and never leaves a reader behind.
/// Pixel width and height of one character cell, from the terminal's
/// window size.
pub(crate) fn cell_pixels() -> Option<(u16, u16)> {
    #[cfg(unix)]
    {
        use std::os::fd::AsRawFd;
        let tty = std::fs::File::open("/dev/tty").ok()?;
        // SAFETY: TIOCGWINSZ fills a winsize struct we own, on an open fd.
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(tty.as_raw_fd(), libc::TIOCGWINSZ, &raw mut ws) } != 0 {
            return None;
        }
        (ws.ws_xpixel > 0 && ws.ws_ypixel > 0 && ws.ws_col > 0 && ws.ws_row > 0)
            .then(|| (ws.ws_xpixel / ws.ws_col, ws.ws_ypixel / ws.ws_row))
    }
    #[cfg(not(unix))]
    None
}

#[cfg(unix)]
fn ask(query: &[u8], timeout: Duration, done: impl Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
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
    if tty.write_all(query).is_ok() && tty.flush().is_ok() {
        let deadline = std::time::Instant::now() + timeout;
        let mut buf = [0u8; 256];
        while !done(&reply) {
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
    // A reply cut off by the timeout would be read later as keystrokes;
    // TCSAFLUSH drops whatever of it has arrived.
    let when = if done(&reply) {
        libc::TCSANOW
    } else {
        libc::TCSAFLUSH
    };
    unsafe { libc::tcsetattr(fd, when, &raw const saved) };
    Some(reply)
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
        #[allow(
            clippy::cast_lossless,
            reason = "suseconds_t is i64 on Linux but i32 on macOS, where `From<u32>` doesn't exist"
        )]
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
fn ask(_query: &[u8], _timeout: Duration, _done: impl Fn(&[u8]) -> bool) -> Option<Vec<u8>> {
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

    #[test]
    fn parses_graphics_replies() {
        // Ghostty-like: kitty graphics, cell size, no sixel.
        let g = parse_graphics(b"\x1b_Gi=31;OK\x1b\\\x1b[6;20;10t\x1b[?62;22c");
        assert_eq!(
            g,
            Graphics {
                kitty: true,
                sixel: false,
                cell: Some((10, 20))
            }
        );
        // A sixel terminal without the kitty protocol or a cell size.
        let g = parse_graphics(b"\x1b[?62;4;9;22c");
        assert!(!g.kitty && g.sixel && g.cell.is_none());
        // Kitty's error reply is not support.
        assert!(!parse_graphics(b"\x1b_Gi=31;ENOTSUPPORTED\x1b\\\x1b[?62c").kitty);
        assert_eq!(da1_end(b"junk\x1b[?62;4c"), Some(11));
        assert_eq!(da1_end(b"\x1b[?62;4"), None);
    }
}
