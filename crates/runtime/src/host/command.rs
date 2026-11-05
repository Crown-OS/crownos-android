use std::process::{Command, Stdio};
use std::time::Duration;

use super::{HostError, wait_for};

/// Runs `program` with `args`, failing when it exits non-zero or outlives
/// `timeout`.
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<(), HostError> {
    let command_line = || format!("{program} {}", args.join(" "));
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| HostError::Command {
            command: command_line(),
            detail: error.to_string(),
        })?;
    let waited = wait_for(&command_line(), timeout, || Ok(child.try_wait()?));
    let status = match waited {
        Ok(status) => status,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    if status.success() {
        Ok(())
    } else {
        Err(HostError::Command {
            command: command_line(),
            detail: status.to_string(),
        })
    }
}
