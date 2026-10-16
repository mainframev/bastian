//! Terminal queries. These talk to `/dev/tty` directly, never stdin or
//! stdout, so they work when output is piped or redirected.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Background {
    Dark,
    Light,
}

impl Background {
    /// `Light` when `light`, else `Dark`.
    fn light_if(light: bool) -> Self {
        if light { Self::Light } else { Self::Dark }
    }
}

/// How long to wait for the terminal to answer before giving up. Every
/// query ends with a device-attributes request, which all terminals answer,
/// so a responsive terminal ends the wait at once; this bounds slow links
/// and terminals that don't answer at all.
const QUERY_TIMEOUT: Duration = Duration::from_secs(1);

/// The terminal's background brightness: asked with OSC 11 when `query`,
/// else guessed from `COLORFGBG`. `None` when neither says. Only query with
/// a terminal on stdout: behind a pipe the reader (a pager, say) shares
/// `/dev/tty`.
pub fn background(query: bool) -> Option<Background> {
    query
        .then(query_background)
        .flatten()
        .or_else(|| from_colorfgbg(std::env::var("COLORFGBG").ok().as_deref()?))
}

/// Parse an OSC 11 reply such as `\e]11;rgb:1919/1717/2424\e\\`, which may
/// be followed by the device-attributes reply.
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
    Some(Background::light_if(luminance > 0.5))
}

/// `COLORFGBG` is `fg;bg` (or `fg;default;bg`) with ANSI color indices; set by
/// rxvt, Konsole and some others. Backgrounds 7 and 9–15 are light.
fn from_colorfgbg(value: &str) -> Option<Background> {
    let bg: u8 = value.rsplit(';').next()?.trim().parse().ok()?;
    Some(Background::light_if(bg == 7 || (9..=15).contains(&bg)))
}

/// Ask OSC 11, then the device attributes: terminals answer in order, so the
/// DA1 reply marks the end, and a terminal that ignores OSC 11 still ends
/// the wait at once.
fn query_background() -> Option<Background> {
    const QUERY: &[u8] = b"\x1b]11;?\x1b\\\x1b[c";
    let reply = ask(QUERY, QUERY_TIMEOUT, |r| da1_end(r).is_some())?;
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
    ask(QUERY, QUERY_TIMEOUT, |r| da1_end(r).is_some())
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

/// Pixel width and height of one character cell, from the terminal's
/// window size.
pub(crate) fn cell_pixels() -> Option<(u16, u16)> {
    let ws = crossterm::terminal::window_size().ok()?;
    (ws.width > 0 && ws.height > 0 && ws.columns > 0 && ws.rows > 0)
        .then(|| (ws.width / ws.columns, ws.height / ws.rows))
}

/// Write `query` to the terminal and read its reply until `done` says it's
/// complete or `timeout` passes. Talks to `/dev/tty` in raw mode, always
/// restored, and never leaves a reader behind.
///
/// Does nothing unless this process is in the terminal's foreground
/// process group: a background job changing the terminal mode would be
/// stopped by `SIGTTOU`.
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

    // SAFETY: tcgetpgrp on a file descriptor we own; getpgrp can't fail.
    if unsafe { libc::tcgetpgrp(fd) != libc::getpgrp() } {
        return None;
    }

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
        // Followed by the device-attributes reply that ends the query.
        assert_eq!(
            parse_osc11("\x1b]11;rgb:fafa/f4f4/eded\x1b\\\x1b[?62;22c"),
            Some(Background::Light)
        );
        assert_eq!(
            parse_osc11("\x1b]11;rgb:1919/1717/2424\x07\x1b[?1;2c"),
            Some(Background::Dark)
        );
        // A terminal that ignores OSC 11 only answers the device attributes.
        assert_eq!(parse_osc11("\x1b[?62;22c"), None);
        assert_eq!(parse_osc11("garbage"), None);
        assert_eq!(parse_osc11(""), None);
    }

    #[test]
    fn background_query_ends_on_device_attributes() {
        // The OSC 11 reply alone isn't the end; the DA1 reply after it is.
        assert_eq!(da1_end(b"\x1b]11;rgb:1919/1717/2424\x1b\\"), None);
        assert_eq!(
            da1_end(b"\x1b]11;rgb:1919/1717/2424\x1b\\\x1b[?62;22c"),
            Some(33)
        );
        assert_eq!(da1_end(b"\x1b[?62;22c"), Some(8));
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

    /// Selects the role a re-invoked test binary plays in
    /// `background_job_does_not_query`.
    #[cfg(unix)]
    const PTY_ROLE: &str = "BASTIAN_TERM_PTY_ROLE";
    #[cfg(unix)]
    const PTY_HELPER: &str = "term::tests::pty_helper";

    /// Rerun this test binary as `pty_helper` in `role`.
    #[cfg(unix)]
    fn helper(role: &str) -> std::process::Command {
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.args([PTY_HELPER, "--exact", "--ignored", "--nocapture"])
            .args(["--test-threads", "1"])
            .env(PTY_ROLE, role);
        cmd
    }

    /// A background job asking the terminal must return without touching it:
    /// changing the terminal mode from outside the foreground process group
    /// would stop the job with `SIGTTOU`. Forking inside the multithreaded
    /// test harness isn't sound, so the test binary is rerun as
    /// `pty_helper`: a session leader on a fresh pty (the foreground group)
    /// that starts a second copy in a new, background process group.
    #[cfg(unix)]
    #[test]
    fn background_job_does_not_query() {
        use std::io::Read;
        use std::os::fd::{AsRawFd, FromRawFd};
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::process::CommandExt;

        // SAFETY: posix_openpt returns a new descriptor we take ownership
        // of; fcntl, grantpt and unlockpt act on it. ptsname's static buffer
        // is copied out at once, and no other test calls it.
        let (master, slave_path) = unsafe {
            let fd = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
            assert!(fd >= 0, "posix_openpt failed");
            let master = std::fs::File::from_raw_fd(fd);
            // Keep it out of processes other tests spawn meanwhile.
            assert_eq!(libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC), 0);
            assert_eq!(libc::grantpt(fd), 0, "grantpt failed");
            assert_eq!(libc::unlockpt(fd), 0, "unlockpt failed");
            let name = libc::ptsname(fd);
            assert!(!name.is_null(), "ptsname failed");
            let path = std::ffi::CStr::from_ptr(name).to_str().unwrap().to_owned();
            (master, path)
        };
        // Held open until the master is read, so the pty stays up.
        let slave = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOCTTY)
            .open(&slave_path)
            .unwrap();

        let mut leader = helper("leader");
        leader.stdin(slave.try_clone().unwrap());
        // SAFETY: setsid and ioctl are async-signal-safe and touch only the
        // child: it leaves our session and takes its stdin, the pty slave, as
        // its controlling terminal, which puts its group in the foreground.
        #[allow(
            clippy::cast_lossless,
            clippy::unnecessary_cast,
            reason = "TIOCSCTTY and ioctl's request differ in type on macOS, not on glibc"
        )]
        unsafe {
            leader.pre_exec(|| {
                if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = leader
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        // Read what reaches the terminal while the helper runs: a restore
        // with TCSAFLUSH waits for the output to drain. The master reads
        // end of file once the session leader exits and hangs up.
        let mut written = Vec::new();
        let mut buf = [0u8; 256];
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let finished = loop {
            if readable(master.as_raw_fd(), Duration::from_millis(20)) {
                match (&master).read(&mut buf) {
                    Ok(n) if n > 0 => written.extend_from_slice(&buf[..n]),
                    _ => break true,
                }
            } else if child.try_wait().unwrap().is_some() {
                break true;
            }
            if std::time::Instant::now() > deadline {
                child.kill().unwrap();
                break false;
            }
        };
        let out = child.wait_with_output().unwrap();
        assert!(
            finished && out.status.success(),
            "pty helper {} (terminal got {:?}):\n{}{}",
            if finished { "failed" } else { "didn't finish" },
            String::from_utf8_lossy(&written),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );

        // Only the foreground leader's control query reached the terminal.
        let written = String::from_utf8_lossy(&written);
        assert!(written.contains("FG-QUERY"), "{written:?}");
        assert!(!written.contains("BG-QUERY"), "{written:?}");
        drop(slave);
    }

    /// Run by `background_job_does_not_query` in a child process; a no-op
    /// otherwise.
    #[cfg(unix)]
    #[test]
    #[ignore = "run by background_job_does_not_query"]
    fn pty_helper() {
        match std::env::var(PTY_ROLE).as_deref() {
            Ok("leader") => pty_leader(),
            // A timeout well past the leader's deadline: returning in time
            // means it didn't wait for a reply.
            Ok("background") => {
                assert_eq!(ask(b"BG-QUERY", Duration::from_secs(30), |_| false), None);
            }
            _ => {}
        }
    }

    /// The session leader: start a background job that asks the terminal,
    /// check it finished without being stopped and left the terminal mode
    /// alone, then ask from the foreground as a control.
    #[cfg(unix)]
    fn pty_leader() {
        use std::os::fd::AsRawFd;
        use std::os::unix::process::CommandExt;

        let tty = std::fs::File::open("/dev/tty").unwrap();
        let attrs = || {
            // SAFETY: tcgetattr fills a termios we own, on an open fd.
            let mut t: libc::termios = unsafe { std::mem::zeroed() };
            assert_eq!(unsafe { libc::tcgetattr(tty.as_raw_fd(), &raw mut t) }, 0);
            (t.c_iflag, t.c_oflag, t.c_cflag, t.c_lflag, t.c_cc)
        };
        let before = attrs();

        let mut job = helper("background");
        job.process_group(0);
        // SAFETY: signal is async-signal-safe. Restore SIGTTOU's default
        // (stop) in case it was inherited as ignored, which would hide a
        // missing guard.
        unsafe {
            job.pre_exec(|| {
                libc::signal(libc::SIGTTOU, libc::SIG_DFL);
                Ok(())
            });
        }
        let mut child = job.spawn().unwrap();
        let pid = libc::pid_t::try_from(child.id()).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        // Poll with WUNTRACED, which reports the job if it was stopped
        // rather than exiting; std's wait would block on it forever.
        let status = loop {
            let mut status = 0;
            // SAFETY: waitpid on our own child, with a status int we own.
            let r = unsafe { libc::waitpid(pid, &raw mut status, libc::WNOHANG | libc::WUNTRACED) };
            assert!(r >= 0, "waitpid failed");
            if r == pid {
                break Some(status);
            }
            if std::time::Instant::now() > deadline {
                break None;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        let Some(status) = status.filter(|&s| !libc::WIFSTOPPED(s)) else {
            // Stopped or stuck, and not reaped yet.
            child.kill().unwrap();
            child.wait().unwrap();
            match status {
                Some(s) => panic!("background job stopped by signal {}", libc::WSTOPSIG(s)),
                None => panic!("background job didn't finish"),
            }
        };
        assert!(
            libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
            "background job failed: status {status:#x}"
        );
        assert!(before == attrs(), "terminal mode changed");

        // The control: from the foreground the query is sent, and with no
        // one answering it times out with an empty reply.
        assert_eq!(
            ask(b"FG-QUERY", Duration::from_millis(50), |_| false),
            Some(Vec::new())
        );
    }
}
