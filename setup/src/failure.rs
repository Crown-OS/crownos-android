use setup_state::{Failure, FailureCode, SetupStatus};

/// Whether running setup again, without changing the machine, can succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Retryable {
    Yes,
    No,
}

/// Why setup stopped: what the notification says, and the technical cause
/// that goes to the journal.
#[derive(Debug, thiserror::Error)]
#[error("{code:?}: {error:#}")]
pub struct SetupFailure {
    pub code: FailureCode,
    pub retryable: Retryable,
    pub error: anyhow::Error,
}

impl SetupFailure {
    pub fn new(code: FailureCode, retryable: Retryable, error: anyhow::Error) -> Self {
        Self {
            code,
            retryable,
            error,
        }
    }

    /// The `setup.json` record for this failure.
    pub fn status(&self) -> SetupStatus {
        SetupStatus::failed(Failure {
            code: self.code,
            detail: detail(self.code).to_owned(),
            retryable: self.retryable == Retryable::Yes,
        })
    }
}

const fn detail(code: FailureCode) -> &'static str {
    match code {
        FailureCode::NoBinder => "This system's kernel has no binder support, which Android needs.",
        FailureCode::UnsupportedPageSize => {
            "This system's kernel uses a memory page size Android cannot run on."
        }
        FailureCode::NotEnoughDisk => "Android needs at least 6 GB of free disk space.",
        FailureCode::Network => "The Android download could not finish.",
        FailureCode::Checksum => "The downloaded Android image was damaged.",
        FailureCode::Signature => "The CrownOS Android files are not signed by CrownOS.",
        FailureCode::Init => "Android could not be set up on this system.",
        FailureCode::Bundle => "The CrownOS Android integration could not be installed.",
        FailureCode::Internal => "Android setup stopped because of an internal error.",
    }
}

/// Attaches a [`FailureCode`] to any error on its way out of a setup stage.
pub trait FailWith<T> {
    fn fail_with(self, code: FailureCode, retryable: Retryable) -> Result<T, SetupFailure>;
}

impl<T, E: Into<anyhow::Error>> FailWith<T> for Result<T, E> {
    fn fail_with(self, code: FailureCode, retryable: Retryable) -> Result<T, SetupFailure> {
        self.map_err(|error| SetupFailure::new(code, retryable, error.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_carries_the_code_and_the_retry_hint() {
        let failure = SetupFailure::new(
            FailureCode::Network,
            Retryable::Yes,
            anyhow::anyhow!("connection reset"),
        );
        let status = failure.status();
        assert_eq!(status.stage, setup_state::Stage::Failed);
        let record = status
            .failure
            .as_ref()
            .map(|failure| (failure.code, failure.retryable));
        assert_eq!(record, Some((FailureCode::Network, true)));
    }
}
