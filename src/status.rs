use std::fmt;
use std::process::ExitCode;

/// Extract the exit code from an error, if it carries one.
/// Otherwise, return [`ExitCode::FAILURE`].
pub fn exit_code_of(err: &miette::Error) -> ExitCode {
    err.downcast_ref::<ExitStatus>()
        .map_or(ExitCode::FAILURE, |status| from_code(status.0))
}

/// Whether the error still needs to be reported to the user.
///
/// An [`ExitStatus`] error already conveys its outcome through the exit code
/// alone and must not be printed by the top-level error printer.
pub fn should_report(err: &miette::Error) -> bool {
    err.downcast_ref::<ExitStatus>().is_none()
}

/// Convert a raw exit code into an [`ExitCode`].
///
/// The standard library only exposes a `u8` inner code. Codes outside that
/// range (for example, a massive Windows system error code or a negative
/// exit code) cannot be represented and are reported as [`ExitCode::FAILURE`].
pub fn from_code(code: i32) -> ExitCode {
    u8::try_from(code)
        .map(ExitCode::from)
        .unwrap_or(ExitCode::FAILURE)
}

/// Finish the command with the given exit code and no further output.
pub fn exit_with(code: i32) -> miette::Error {
    ExitStatus(code).into()
}

/// Error carrying an exit code through the command result.
#[derive(Debug)]
struct ExitStatus(i32);

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exited with code {}", self.0)
    }
}

impl std::error::Error for ExitStatus {}

impl miette::Diagnostic for ExitStatus {}
