use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Output lines kept per block; older lines are dropped and counted.
pub const MAX_OUTPUT_LINES: usize = 2000;

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
    tx: Sender<RunEvent>,
    rx: Receiver<RunEvent>,
    pids: HashMap<usize, u32>,
    cancelled: HashSet<usize>,
}

impl Default for Runner {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            tx,
            rx,
            pids: HashMap::new(),
            cancelled: HashSet::new(),
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
            while reader.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
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
            kill_group(pid);
        }
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
                *cancelled = self.cancelled.remove(id);
            }
        }
        events
    }
}

impl Drop for Runner {
    fn drop(&mut self) {
        for &pid in self.pids.values() {
            kill_group(pid);
        }
    }
}

#[cfg(unix)]
fn kill_group(pid: u32) {
    // SAFETY: kill(2) has no memory-safety preconditions. A negative pid
    // targets the process group created for the block in `start`.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGTERM);
    }
}

#[cfg(not(unix))]
fn kill_group(_pid: u32) {}

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
