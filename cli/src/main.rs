//! `crown-android`, the client the desktop entries and the shell run.

mod args;
mod inbox;
mod outcome;

use std::process::ExitCode;
use std::time::Duration;

use android_protocol::{StopReason, android};
use anyhow::{Context, Result};
use runtime::Paths;

use args::Command;

/// How long an ordinary call may take.
const CALL_TIMEOUT: Duration = Duration::from_secs(10);
/// `stop` returns after the whole stop sequence; `diagnose` after the archive.
const SLOW_CALL_TIMEOUT: Duration = Duration::from_secs(150);
const ACTIVATION_TOKEN: &str = "XDG_ACTIVATION_TOKEN";

fn main() -> ExitCode {
    let command = match args::parse(std::env::args().skip(1)) {
        Ok(command) => command,
        Err(usage) => {
            eprintln!("{usage}");
            return ExitCode::from(2);
        }
    };
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("crown-android: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<()> {
    match command {
        Command::Launch { package } => {
            let token = std::env::var(ACTIVATION_TOKEN)
                .ok()
                .filter(|token| !token.is_empty());
            let mut client = connect()?;
            let pending = client.launch(package, token)?;
            outcome::check(client.wait(pending, Some(CALL_TIMEOUT))?)
        }
        Command::Stop => {
            let mut client = connect()?;
            let pending = client.stop(StopReason::User)?;
            Ok(client.wait(pending, Some(SLOW_CALL_TIMEOUT))?)
        }
        Command::Status => {
            let mut client = connect()?;
            let pending = client.status()?;
            print!(
                "{}",
                outcome::describe(&client.wait(pending, Some(CALL_TIMEOUT))?)
            );
            Ok(())
        }
        Command::Diagnose => {
            let mut client = connect()?;
            let pending = client.diagnose()?;
            println!("{}", client.wait(pending, Some(SLOW_CALL_TIMEOUT))?);
            Ok(())
        }
        Command::Install { apk } => {
            let paths = Paths::from_env()?;
            let file_name = inbox::copy_in(&apk, &paths.inbox())?;
            let mut client = connect()?;
            let pending = client.install_package(file_name)?;
            outcome::check(client.wait(pending, Some(CALL_TIMEOUT))?)
        }
        Command::Open { uri } => {
            let mut client = connect()?;
            let pending = client.open_uri(uri)?;
            outcome::check(client.wait(pending, Some(CALL_TIMEOUT))?)
        }
        Command::Settings { package } => {
            let mut client = connect()?;
            let pending = client.open_app_settings(package)?;
            outcome::check(client.wait(pending, Some(CALL_TIMEOUT))?)
        }
    }
}

fn connect() -> Result<android::Client> {
    android::Client::connect().context("crown-androidd is not reachable")
}
