//! Controlled process execution. Every external program (adb, avdmanager,
//! emulator) is started through here so that we get timeouts, captured output
//! and no flashing console windows on Windows.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

/// Sink for user-visible log lines: (level, message). Levels: info, warn, error.
pub type Logger = Arc<dyn Fn(&str, String) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct RunOutput {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl RunOutput {
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout.trim(), self.stderr.trim())
            .trim()
            .to_string()
    }
}

/// A `Command` that never opens a console window on Windows.
pub fn command<S: AsRef<OsStr>>(program: S) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Reads a pipe on a helper thread. The result is delivered over a channel so
/// the caller can stop waiting: a background daemon (the adb server) may keep
/// the pipe open long after the command itself has finished.
fn drain<R: Read + Send + 'static>(mut r: R) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        let _ = tx.send(String::from_utf8_lossy(&buf).into_owned());
    });
    rx
}

pub fn run(
    program: &Path,
    args: &[&str],
    stdin: Option<&str>,
    timeout: Duration,
) -> Result<RunOutput, String> {
    run_env(program, args, stdin, timeout, &[])
}

pub fn run_env(
    program: &Path,
    args: &[&str],
    stdin: Option<&str>,
    timeout: Duration,
    envs: &[(&str, &str)],
) -> Result<RunOutput, String> {
    let mut cmd = command(program);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start {}: {e}", program.display()))?;

    if let Some(text) = stdin {
        if let Some(mut si) = child.stdin.take() {
            let _ = si.write_all(text.as_bytes());
        } // dropped here: closes stdin
    }

    let out_rx = child.stdout.take().map(drain);
    let err_rx = child.stderr.take().map(drain);

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!(
                        "{} timed out after {}s",
                        program
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "command".into()),
                        timeout.as_secs()
                    ));
                }
                thread::sleep(Duration::from_millis(30));
            }
            Err(e) => return Err(format!("Failed waiting for process: {e}")),
        }
    };

    let stdout = out_rx
        .and_then(|rx| rx.recv_timeout(Duration::from_secs(2)).ok())
        .unwrap_or_default();
    let stderr = err_rx
        .and_then(|rx| rx.recv_timeout(Duration::from_millis(500)).ok())
        .unwrap_or_default();

    Ok(RunOutput {
        code: status.code(),
        stdout,
        stderr,
    })
}
