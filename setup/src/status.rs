use std::path::PathBuf;

use setup_state::{SetupStatus, Stage};

/// Where setup reports its progress.
pub trait StatusSink {
    fn report(&mut self, status: SetupStatus) -> anyhow::Result<()>;

    fn stage(&mut self, stage: Stage, percent: Option<u8>, message: &str) -> anyhow::Result<()> {
        self.report(SetupStatus::new(stage, percent, message))
    }
}

/// `setup.json`, rewritten through [`setup_state::store`] only when the
/// record changes, so a download writes it at most once per percent.
pub struct StatusFile {
    path: PathBuf,
    last: Option<SetupStatus>,
}

impl StatusFile {
    pub fn new(path: PathBuf) -> Self {
        Self { path, last: None }
    }
}

impl StatusSink for StatusFile {
    fn report(&mut self, status: SetupStatus) -> anyhow::Result<()> {
        if self.last.as_ref() == Some(&status) {
            return Ok(());
        }
        setup_state::store(&self.path, &status)?;
        self.last = Some(status);
        Ok(())
    }
}

/// Records every report, for tests that check the order of stages.
impl StatusSink for Vec<SetupStatus> {
    fn report(&mut self, status: SetupStatus) -> anyhow::Result<()> {
        if self.last() != Some(&status) {
            self.push(status);
        }
        Ok(())
    }
}
