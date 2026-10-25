# Return an exit code from `main` instead of terminating the process

The CLI binaries used to terminate the process directly with
`std::process::exit`. `cargo llvm-cov` cannot collect coverage for code paths
that end in `process::exit`, because the process is torn down before the
instrumentation flushes. The binaries now return `std::process::ExitCode` from
`main`, and command handlers surface exit intent as an error variant that the
top level converts into an `ExitCode`.

## Considered Options

- **Keep `process::exit`** — rejected: the coverage gap on every error path is
  the problem being fixed.
- **Return `ExitCode` only from `main`, keeping early exits in handlers** —
  rejected: partial adoption would leave most failure paths untestable.
- **A silent-exit error variant carrying the requested code** — chosen: it
  keeps handlers returning `miette::Result<()>`, preserves the exact codes
  (including `2` for the help fallback), and lets the top level decide both
  the code and whether to print.

## Consequences

- `src/status.rs` owns the conversion: `exit_code_of` maps an error to an
  `ExitCode`, `from_code` converts a raw exit code, `exit_with` returns a
  silent-exit error, and `should_report` lets the top level skip printing it.
- Handlers keep their `miette::Result<()>` signatures, so their logic stays
  reachable and testable.
- Child process exit codes propagate from `moonup run` and `moonup-shim`;
  codes that do not fit the standard library's `u8` inner code degrade to
  `FAILURE`.
- Handler output is unchanged: the help-fallback cases still print help to
  stdout and exit `2` with no extra stderr.
