use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, SyncSender};
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
    pids: HashMap<usize, u32>,
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
            pids: HashMap::new(),
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
        self.pids.insert(id, child.id());
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
            let code = child.wait().ok().and_then(|s| s.code());
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
            let mut reader = BufReader::new(stream);
            let mut buf = Vec::new();
            while reader
                .by_ref()
                .take(MAX_LINE_BYTES as u64)
                .read_until(b'\n', &mut buf)
                .is_ok_and(|n| n > 0)
            {
                let text = String::from_utf8_lossy(&buf);
                let text = text.trim_end_matches(['\n', '\r']);
                // Keep only the last carriage-return segment, as a terminal would show.
                let text = text.rsplit('\r').next().unwrap_or("").to_string();
                if tx.send(RunEvent::Output { id, text, stderr }).is_err() {
                    return;
                }
                buf.clear();
            }
        })
    }

    pub fn cancel(&mut self, id: usize) {
        if let Some(&pid) = self.pids.get(&id) {
            self.cancelled.insert(id);
            self.term_sent.entry(id).or_insert_with(Instant::now);
            kill_group(pid, Signal::Term);
        }
    }

    /// SIGKILL cancelled blocks still running `KILL_GRACE` after SIGTERM,
    /// once per block.
    pub fn escalate(&mut self) {
        let pids = &self.pids;
        self.term_sent.retain(|id, sent| {
            let Some(&pid) = pids.get(id) else {
                return false;
            };
            if sent.elapsed() < KILL_GRACE {
                return true;
            }
            kill_group(pid, Signal::Kill);
            false
        });
    }

    pub fn is_running(&self, id: usize) -> bool {
        self.pids.contains_key(&id)
    }

    /// Events received since the last call, without blocking.
    pub fn drain(&mut self) -> Vec<RunEvent> {
        let mut events: Vec<RunEvent> = self.rx.try_iter().collect();
        for e in &mut events {
            if let RunEvent::Exit { id, cancelled, .. } = e {
                self.pids.remove(id);
                self.term_sent.remove(id);
                *cancelled = self.cancelled.remove(id);
            }
        }
        events
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        if self.pids.is_empty() {
            return;
        }
        for &pid in self.pids.values() {
            kill_group(pid, Signal::Term);
        }
        // A short grace period to clean up, then stop whatever ignored it.
        thread::sleep(Duration::from_millis(100));
        for &pid in self.pids.values() {
            kill_group(pid, Signal::Kill);
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
        let pid = r.pids[&0] as libc::pid_t;
        drop(r);
        let deadline = Instant::now() + Duration::from_secs(2);
        // SAFETY: kill(2) with signal 0 only checks that the group exists.
        while unsafe { libc::kill(-pid, 0) } == 0 {
            assert!(Instant::now() < deadline, "block survived drop");
            thread::sleep(Duration::from_millis(10));
        }
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
