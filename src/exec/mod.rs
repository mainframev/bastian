use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

/// Output lines kept per block; older lines are dropped and counted.
const MAX_OUTPUT_LINES: usize = 2000;

/// Longest output line read in one piece; a longer run without a newline
/// is split into lines of this size, so memory stays bounded.
const MAX_LINE_BYTES: usize = 64 * 1024;

/// Events buffered before the reader threads block, which in turn stalls
/// a block that writes faster than the UI drains.
const CHANNEL_CAPACITY: usize = 4096;

/// How long a cancelled block gets to exit after SIGTERM before SIGKILL.
const KILL_GRACE: Duration = Duration::from_secs(2);

/// Built-in command lines that run a block's source for a fence language.
/// The source is passed as the final argument.
fn builtin(lang: &str) -> Option<&'static [&'static str]> {
    Some(match lang {
        "sh" => &["sh", "-c"],
        "bash" => &["bash", "-c"],
        "zsh" => &["zsh", "-c"],
        "fish" => &["fish", "-c"],
        "python" | "py" | "python3" => &["python3", "-c"],
        "node" | "js" | "javascript" => &["node", "-e"],
        _ => return None,
    })
}

/// Built-in interpreters plus overrides from the config file.
#[derive(Debug, Clone, Default)]
pub struct Interpreters {
    overrides: HashMap<String, Vec<String>>,
}

impl Interpreters {
    pub fn new(overrides: HashMap<String, Vec<String>>) -> Self {
        Self { overrides }
    }

    /// Command line for `lang`, without the source. An empty override
    /// disables a built-in language.
    pub fn get(&self, lang: &str) -> Option<Vec<String>> {
        match self.overrides.get(lang) {
            Some(cmd) if cmd.is_empty() => None,
            Some(cmd) => Some(cmd.clone()),
            None => builtin(lang).map(|c| c.iter().map(ToString::to_string).collect()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RunSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, PartialEq)]
pub enum RunEvent {
    Output {
        id: usize,
        text: String,
        stderr: bool,
    },
    Exit {
        id: usize,
        code: Option<i32>,
        cancelled: bool,
        took: Duration,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Running(Instant),
    Ok(Duration),
    Failed { code: Option<i32>, took: Duration },
    Cancelled(Duration),
    TimedOut(Duration),
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutputLine {
    pub text: String,
    pub stderr: bool,
}

/// Run state of one code block, as shown under it.
#[derive(Debug, Clone)]
pub struct BlockRun {
    pub status: Status,
    pub output: VecDeque<OutputLine>,
    /// Lines dropped from the front to stay under `MAX_OUTPUT_LINES`.
    pub dropped: usize,
    pub expanded: bool,
}

impl BlockRun {
    pub fn started() -> Self {
        Self {
            status: Status::Running(Instant::now()),
            output: VecDeque::new(),
            dropped: 0,
            expanded: true,
        }
    }

    pub fn push(&mut self, text: String, stderr: bool) {
        if self.output.len() == MAX_OUTPUT_LINES {
            self.output.pop_front();
            self.dropped += 1;
        }
        self.output.push_back(OutputLine { text, stderr });
    }

    pub fn is_running(&self) -> bool {
        matches!(self.status, Status::Running(_))
    }
}

/// Spawns block processes and streams their output over a channel.
/// Dropping the runner kills everything still running.
pub struct Runner {
    tx: SyncSender<RunEvent>,
    rx: Receiver<RunEvent>,
    procs: HashMap<usize, Proc>,
    cancelled: HashSet<usize>,
    /// When SIGTERM went to each cancelled block still awaiting SIGKILL.
    term_sent: HashMap<usize, Instant>,
}

impl Default for Runner {
    fn default() -> Self {
        let (tx, rx) = mpsc::sync_channel(CHANNEL_CAPACITY);
        Self {
            tx,
            rx,
            procs: HashMap::new(),
            cancelled: HashSet::new(),
            term_sent: HashMap::new(),
        }
    }
}

impl Runner {
    pub fn start(&mut self, id: usize, spec: &RunSpec) -> std::io::Result<()> {
        let mut cmd = Command::new(&spec.program);
        cmd.args(&spec.args)
            .current_dir(&spec.cwd)
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Own process group, so cancelling also stops the block's children.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

        let started = Instant::now();
        let mut child = cmd.spawn()?;
        let reaped = Arc::new(Mutex::new(false));
        self.procs.insert(
            id,
            Proc {
                pid: child.id(),
                reaped: Arc::clone(&reaped),
            },
        );
        self.cancelled.remove(&id);
        self.term_sent.remove(&id);

        let readers = [
            child.stdout.take().map(|s| self.pipe(id, s, false)),
            child.stderr.take().map(|s| self.pipe(id, s, true)),
        ];
        let tx = self.tx.clone();
        thread::spawn(move || {
            for r in readers.into_iter().flatten() {
                let _ = r.join();
            }
            // Wait for the leader to exit but leave it a zombie, so its pid
            // and process group can't be reused while `reaped` is false.
            #[cfg(unix)]
            wait_exit_unreaped(child.id());
            let code = {
                let mut reaped = reaped.lock().unwrap_or_else(PoisonError::into_inner);
                *reaped = true;
                child.wait().ok().and_then(|s| s.code())
            };
            let _ = tx.send(RunEvent::Exit {
                id,
                code,
                cancelled: false,
                took: started.elapsed(),
            });
        });
        Ok(())
    }

    fn pipe(
        &self,
        id: usize,
        stream: impl Read + Send + 'static,
        stderr: bool,
    ) -> thread::JoinHandle<()> {
        let tx = self.tx.clone();
        thread::spawn(move || {
            for text in LineSplitter::new(BufReader::new(stream), MAX_LINE_BYTES) {
                if tx.send(RunEvent::Output { id, text, stderr }).is_err() {
                    return;
                }
            }
        })
    }

    pub fn cancel(&mut self, id: usize) {
        if let Some(proc) = self.procs.get(&id) {
            self.cancelled.insert(id);
            self.term_sent.entry(id).or_insert_with(Instant::now);
            proc.signal(Signal::Term);
        }
    }

    /// SIGKILL cancelled blocks still running `KILL_GRACE` after SIGTERM,
    /// once per block.
    pub fn escalate(&mut self) {
        let procs = &self.procs;
        self.term_sent.retain(|id, sent| {
            let Some(proc) = procs.get(id) else {
                return false;
            };
            if sent.elapsed() < KILL_GRACE {
                return true;
            }
            proc.signal(Signal::Kill);
            false
        });
    }

    pub fn is_running(&self, id: usize) -> bool {
        self.procs.contains_key(&id)
    }

    /// Events received since the last call, without blocking.
    pub fn drain(&mut self) -> Vec<RunEvent> {
        let mut events: Vec<RunEvent> = self.rx.try_iter().collect();
        for e in &mut events {
            if let RunEvent::Exit { id, cancelled, .. } = e {
                self.procs.remove(id);
                self.term_sent.remove(id);
                *cancelled = self.cancelled.remove(id);
            }
        }
        events
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        let mut signalled = false;
        for proc in self.procs.values() {
            signalled |= proc.signal(Signal::Term);
        }
        if !signalled {
            return;
        }
        // A short grace period to clean up, then stop whatever ignored it.
        thread::sleep(Duration::from_millis(100));
        for proc in self.procs.values() {
            proc.signal(Signal::Kill);
        }
    }
}

/// Splits a stream into output lines of at most `cap` bytes. A longer run
/// without a newline becomes several lines, cut between characters.
struct LineSplitter<R> {
    reader: R,
    cap: usize,
    /// Start of a character cut off at the end of the previous chunk.
    carry: Vec<u8>,
    /// The previous chunk was cut at `cap`, not ended by a newline.
    continued: bool,
}

impl<R: BufRead> LineSplitter<R> {
    fn new(reader: R, cap: usize) -> Self {
        // Room for a whole character, so every chunk makes progress.
        debug_assert!(cap >= 4);
        Self {
            reader,
            cap,
            carry: Vec::new(),
            continued: false,
        }
    }
}

impl<R: BufRead> Iterator for LineSplitter<R> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        loop {
            let mut buf = std::mem::take(&mut self.carry);
            let limit = self.cap.saturating_sub(buf.len()).max(1) as u64;
            let read = self.reader.by_ref().take(limit).read_until(b'\n', &mut buf);
            if !read.is_ok_and(|n| n > 0) {
                // End of stream: whatever was carried is all there is.
                return (!buf.is_empty()).then(|| String::from_utf8_lossy(&buf).into_owned());
            }
            // A cut that landed just before the line end already emitted the line.
            if std::mem::replace(&mut self.continued, !buf.ends_with(b"\n"))
                && matches!(buf.as_slice(), b"\n" | b"\r\n")
            {
                continue;
            }
            let text = decode(&buf, &mut self.carry);
            if text.is_empty() && !self.carry.is_empty() {
                continue;
            }
            let text = text.trim_end_matches(['\n', '\r']);
            // Keep only the last carriage-return segment, as a terminal would show.
            return Some(text.rsplit('\r').next().unwrap_or("").to_string());
        }
    }
}

/// Decode `buf` like `String::from_utf8_lossy`, except that a character
/// cut off at the end goes to `carry` instead of becoming U+FFFD.
fn decode(buf: &[u8], carry: &mut Vec<u8>) -> String {
    let mut out = String::with_capacity(buf.len());
    let mut rest = buf;
    loop {
        match std::str::from_utf8(rest) {
            Ok(s) => {
                out.push_str(s);
                return out;
            }
            Err(e) => {
                let (valid, after) = rest.split_at(e.valid_up_to());
                // Valid by `valid_up_to`, so this borrows without replacing.
                out.push_str(&String::from_utf8_lossy(valid));
                let Some(len) = e.error_len() else {
                    carry.extend_from_slice(after);
                    return out;
                };
                out.push(char::REPLACEMENT_CHARACTER);
                rest = &after[len..];
            }
        }
    }
}

/// A block's process group leader, and whether the waiter thread has
/// reaped it.
struct Proc {
    pid: u32,
    reaped: Arc<Mutex<bool>>,
}

impl Proc {
    /// Signal the block's process group unless the leader was reaped.
    /// Until then the leader is at worst a zombie, which keeps its pid
    /// and group id from being reused, so the signal can't reach an
    /// unrelated process. Returns whether the signal was sent.
    fn signal(&self, sig: Signal) -> bool {
        let reaped = self.reaped.lock().unwrap_or_else(PoisonError::into_inner);
        if *reaped {
            return false;
        }
        kill_group(self.pid, sig);
        #[cfg(test)]
        tests::SIGNALS.with(|n| n.set(n.get() + 1));
        true
    }
}

/// Block until process `pid` exits, without reaping it.
#[cfg(unix)]
fn wait_exit_unreaped(pid: u32) {
    loop {
        // SAFETY: an all-zero siginfo_t is a valid value of this plain C struct.
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid, writable siginfo_t for the call. WNOWAIT
        // leaves the child waitable, so `Child::wait` still reaps it.
        let rc = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &raw mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        };
        if rc == 0 || std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return;
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Signal {
    Term,
    Kill,
}

#[cfg(unix)]
fn kill_group(pid: u32, sig: Signal) {
    let sig = match sig {
        Signal::Term => libc::SIGTERM,
        Signal::Kill => libc::SIGKILL,
    };
    // SAFETY: kill(2) has no memory-safety preconditions. A negative pid
    // targets the process group created for the block in `start`.
    unsafe {
        libc::kill(-(pid as libc::pid_t), sig);
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32, _sig: Signal) {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        /// Signals sent by `Proc::signal` on this test's thread.
        pub(super) static SIGNALS: Cell<usize> = const { Cell::new(0) };
    }

    fn spec(src: &str) -> RunSpec {
        RunSpec {
            program: "sh".into(),
            args: vec!["-c".into(), src.into()],
            cwd: std::env::temp_dir(),
            env: Vec::new(),
        }
    }

    /// Collect events for block 0 until it exits.
    fn run_until_exit(runner: &mut Runner, limit: Duration) -> (Vec<String>, RunEvent) {
        let deadline = Instant::now() + limit;
        let mut out = Vec::new();
        while Instant::now() < deadline {
            for e in runner.drain() {
                match e {
                    RunEvent::Output { text, .. } => out.push(text),
                    exit @ RunEvent::Exit { .. } => return (out, exit),
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("block did not exit within {limit:?}; output so far: {out:?}");
    }

    #[test]
    fn streams_output_and_exit_code() {
        let mut r = Runner::default();
        r.start(0, &spec("echo hi; echo err >&2; exit 3")).unwrap();
        let (mut out, exit) = run_until_exit(&mut r, Duration::from_secs(5));
        out.sort();
        assert_eq!(out, ["err", "hi"]);
        assert!(matches!(
            exit,
            RunEvent::Exit {
                code: Some(3),
                cancelled: false,
                ..
            }
        ));
        assert!(!r.is_running(0));
    }

    #[test]
    fn cancel_kills_process_group() {
        let mut r = Runner::default();
        // The child sleep keeps stdout open, so only a group kill ends the run quickly.
        r.start(0, &spec("sleep 30 & sleep 30")).unwrap();
        thread::sleep(Duration::from_millis(100));
        r.cancel(0);
        assert_eq!(SIGNALS.with(Cell::get), 1);
        let (_, exit) = run_until_exit(&mut r, Duration::from_secs(2));
        assert!(matches!(
            exit,
            RunEvent::Exit {
                cancelled: true,
                ..
            }
        ));
    }

    #[test]
    fn escalate_kills_blocks_that_ignore_sigterm() {
        let mut r = Runner::default();
        r.start(0, &spec("trap '' TERM; sleep 30")).unwrap();
        thread::sleep(Duration::from_millis(100));
        r.cancel(0);
        let started = Instant::now();
        let deadline = started + Duration::from_secs(4);
        let exit = 'wait: loop {
            assert!(Instant::now() < deadline, "block survived SIGKILL");
            r.escalate();
            for e in r.drain() {
                if let exit @ RunEvent::Exit { .. } = e {
                    break 'wait exit;
                }
            }
            thread::sleep(Duration::from_millis(10));
        };
        assert!(started.elapsed() >= KILL_GRACE);
        assert!(started.elapsed() < KILL_GRACE + Duration::from_secs(1));
        assert!(matches!(
            exit,
            RunEvent::Exit {
                cancelled: true,
                ..
            }
        ));
        assert!(!r.is_running(0));
    }

    #[cfg(unix)]
    #[test]
    fn drop_kills_blocks_that_ignore_sigterm() {
        let mut r = Runner::default();
        r.start(0, &spec("trap '' TERM; sleep 30")).unwrap();
        thread::sleep(Duration::from_millis(100));
        let pid = r.procs[&0].pid as libc::pid_t;
        drop(r);
        let deadline = Instant::now() + Duration::from_secs(2);
        // SAFETY: kill(2) with signal 0 only checks that the group exists.
        while unsafe { libc::kill(-pid, 0) } == 0 {
            assert!(Instant::now() < deadline, "block survived drop");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn reaped_blocks_are_never_signalled() {
        let mut r = Runner::default();
        r.start(0, &spec("true")).unwrap();
        let reaped = Arc::clone(&r.procs[&0].reaped);
        let deadline = Instant::now() + Duration::from_secs(5);
        // Wait for the waiter thread to reap the block, without draining
        // its exit, so the runner still holds the stale pid.
        while !*reaped.lock().unwrap() {
            assert!(Instant::now() < deadline, "block was not reaped");
            thread::sleep(Duration::from_millis(10));
        }
        assert!(r.is_running(0));

        r.cancel(0);
        r.term_sent
            .insert(0, Instant::now().checked_sub(KILL_GRACE).unwrap());
        r.escalate();
        let started = Instant::now();
        drop(r);
        assert_eq!(SIGNALS.with(Cell::get), 0);
        assert!(started.elapsed() < Duration::from_millis(100));
    }

    #[test]
    fn applies_cwd_and_env() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = spec("pwd; echo \"$GREETING\"");
        s.cwd = dir.path().to_path_buf();
        s.env = vec![("GREETING".into(), "hello".into())];
        let mut r = Runner::default();
        r.start(0, &s).unwrap();
        let (out, _) = run_until_exit(&mut r, Duration::from_secs(5));
        let want = std::fs::canonicalize(dir.path()).unwrap();
        assert_eq!(std::fs::canonicalize(&out[0]).unwrap(), want);
        assert_eq!(out[1], "hello");
    }

    #[test]
    fn carriage_returns_keep_last_segment() {
        let mut r = Runner::default();
        r.start(0, &spec("printf '10%%\\r50%%\\r100%%\\n'"))
            .unwrap();
        let (out, _) = run_until_exit(&mut r, Duration::from_secs(5));
        assert_eq!(out, ["100%"]);
    }

    #[test]
    fn long_lines_are_split() {
        let mut r = Runner::default();
        r.start(0, &spec("yes | tr -d '\\n' | head -c 300000"))
            .unwrap();
        let (out, exit) = run_until_exit(&mut r, Duration::from_secs(10));
        assert!(out.iter().all(|l| l.len() <= MAX_LINE_BYTES));
        assert_eq!(out.iter().map(String::len).sum::<usize>(), 300_000);
        assert!(matches!(exit, RunEvent::Exit { code: Some(0), .. }));
    }

    fn split(input: &[u8], cap: usize) -> Vec<String> {
        LineSplitter::new(input, cap).collect()
    }

    #[test]
    fn line_of_exactly_cap_bytes_is_one_line() {
        assert_eq!(split(b"12345678\nab\n", 8), ["12345678", "ab"]);
        assert_eq!(split(b"12345678\r\nab", 8), ["12345678", "ab"]);
        assert_eq!(split(b"1234567\r\nab", 8), ["1234567", "ab"]);
        assert_eq!(split(b"12345678abc\n", 8), ["12345678", "abc"]);
        assert_eq!(split(b"\n\n", 8), ["", ""]);
        assert_eq!(split(b"12345678\n\n", 8), ["12345678", ""]);
    }

    #[test]
    fn characters_straddling_the_cap_stay_whole() {
        assert_eq!(split("1234567é89\n".as_bytes(), 8), ["1234567", "é89"]);
        assert_eq!(split("123456日\n".as_bytes(), 8), ["123456", "日"]);
        assert_eq!(split("1234567é".as_bytes(), 8), ["1234567", "é"]);
        let text = "日本語のテキスト".repeat(3);
        let lines = split(text.as_bytes(), 8);
        assert!(lines.iter().all(|l| l.len() <= 8));
        assert_eq!(lines.concat(), text);
    }

    #[test]
    fn invalid_utf8_is_replaced() {
        assert_eq!(split(b"a\xffb\n", 8), ["a\u{FFFD}b"]);
        // A cut-off character at the end of the stream is still shown.
        assert_eq!(split(b"ab\xe6\x97", 8), ["ab", "\u{FFFD}"]);
        assert_eq!(split(b"ab\xe6\x97\n", 8), ["ab\u{FFFD}"]);
    }

    #[test]
    fn interpreter_overrides() {
        let i = Interpreters::new(HashMap::from([
            (
                "ruby".to_string(),
                vec!["ruby".to_string(), "-e".to_string()],
            ),
            ("fish".to_string(), Vec::new()),
        ]));
        assert_eq!(i.get("ruby").unwrap(), ["ruby", "-e"]);
        assert_eq!(i.get("fish"), None);
        assert_eq!(i.get("sh").unwrap(), ["sh", "-c"]);
        assert_eq!(i.get("rust"), None);
    }

    #[test]
    fn output_is_capped() {
        let mut b = BlockRun::started();
        for i in 0..MAX_OUTPUT_LINES + 5 {
            b.push(i.to_string(), false);
        }
        assert_eq!(b.output.len(), MAX_OUTPUT_LINES);
        assert_eq!(b.dropped, 5);
        assert_eq!(b.output[0].text, "5");
    }
}
