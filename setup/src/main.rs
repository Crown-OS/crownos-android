//! `crownos-android-setup`, the root setup service (contract 12).

use std::process::ExitCode;

use android_setup::cli::{self, Command};
use android_setup::commands::{enable, make_manifest, prepare_container, remove, run};
use android_setup::{Paths, StatusFile, SystemHost, util};

const USAGE_EXIT: u8 = 2;

fn main() -> ExitCode {
    util::journal::init();
    let command = match cli::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(USAGE_EXIT);
        }
    };
    match execute(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            log::error!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn execute(command: Command) -> anyhow::Result<()> {
    let paths = Paths::system();
    match command {
        Command::Run => {
            let mut status = StatusFile::new(paths.status());
            run::run(&run::Context::system(&paths, &SystemHost), &mut status).map(drop)
        }
        Command::PrepareContainer => prepare_container::prepare_container(&paths),
        Command::Remove(user_data) => remove::remove(&paths, &SystemHost, user_data),
        Command::Enable => enable::enable(&paths, &SystemHost),
        Command::MakeManifest {
            root,
            version,
            pins,
        } => {
            let manifest = make_manifest::make_manifest(&root, &version, &pins)?;
            println!("{}", manifest.display());
            Ok(())
        }
    }
}
