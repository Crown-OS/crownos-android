use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

/// How often a running command is checked for having exited.
const EXIT_POLL: Duration = Duration::from_millis(5);

/// A command that exited, with what it printed and how long it ran.
#[derive(Debug)]
pub struct Finished {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
}

impl Finished {
    pub fn success(&self) -> bool {
        self.status.success()
    }

    /// The last line of stderr, where Python tracebacks put the error, or the exit status.
    pub fn error(&self) -> String {
        self.stderr
            .lines()
            .rev()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map_or_else(|| self.status.to_string(), str::to_owned)
    }
}

/// A command started without waiting for it. Its output must fit the pipe
/// buffer, which every command the bench runs this way does.
pub struct Running {
    child: Child,
    program: String,
    started: Instant,
}

impl Running {
    pub fn spawn(program: &str, args: &[&str]) -> Result<Self> {
        let started = Instant::now();
        let child = Command::new(program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| format!("could not start {program}"))?;
        Ok(Self {
            child,
            program: program.to_owned(),
            started,
        })
    }

    /// The outcome if it has exited, without waiting.
    pub fn try_finish(&mut self) -> Result<Option<Finished>> {
        let Some(status) = self.child.try_wait()? else {
            return Ok(None);
        };
        Ok(Some(Finished {
            status,
            stdout: read_pipe(self.child.stdout.take()),
            stderr: read_pipe(self.child.stderr.take()),
            elapsed: self.started.elapsed(),
        }))
    }

    /// Waits until `timeout` after the start, then kills it.
    pub fn finish(mut self, timeout: Duration) -> Result<Finished> {
        loop {
            if let Some(finished) = self.try_finish()? {
                return Ok(finished);
            }
            if self.started.elapsed() >= timeout {
                self.kill();
                bail!(
                    "{} did not finish within {} s",
                    self.program,
                    timeout.as_secs_f64()
                );
            }
            thread::sleep(EXIT_POLL);
        }
    }

    pub fn kill(&mut self) {
        self.child.kill().ok();
        self.child.wait().ok();
    }
}

fn read_pipe(pipe: Option<impl Read>) -> String {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        pipe.read_to_end(&mut bytes).ok();
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Runs a command to completion, killing it after `timeout`.
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<Finished> {
    Running::spawn(program, args)?.finish(timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(10);

    #[test]
    fn captures_output_and_errors() -> Result<()> {
        let done = run(
            "sh",
            &["-c", "echo ready; echo 'Error: no session' >&2; exit 3"],
            TIMEOUT,
        )?;
        assert!(!done.success());
        assert_eq!(done.stdout, "ready\n");
        assert_eq!(done.error(), "Error: no session");
        let quiet = run("false", &[], TIMEOUT)?;
        assert_eq!(quiet.error(), quiet.status.to_string());
        Ok(())
    }

    #[test]
    fn kills_what_overruns_its_timeout() {
        let started = Instant::now();
        assert!(run("sleep", &["30"], Duration::from_millis(50)).is_err());
        assert!(started.elapsed() < TIMEOUT);
        assert!(run("/nonexistent/crown-android", &[], TIMEOUT).is_err());
    }
}
