# Use one polyglot installer for POSIX and PowerShell

Moonup's public installer is a single extensionless `install` script that can
be piped to either a POSIX shell or PowerShell. The script uses a tested
interpreter-escape preamble so each interpreter evaluates only its own branch.

## Considered Options

- **Keep separate `install.sh` and PowerShell scripts** — rejected: this would
  require users and documentation to choose between URLs and would duplicate
  the installer contract.
- **Use a POSIX-only installer with Windows-specific package-manager guidance**
  — rejected: PowerShell users expect the documented `irm | iex` workflow and
  need a native release fallback.
- **Use the tested polyglot wrapper** — chosen: it preserves the simple pipe
  invocation while allowing native PowerShell package managers and archive
  handling.

## Consequences

- POSIX installation tries Homebrew, cargo-binstall, Pixi, Cargo, then GitHub
  Releases; Windows tries Winget, Scoop, cargo-binstall, Pixi, Cargo, then
  GitHub Releases.
- An available installer is fail-fast: if its command runs and fails, later
  methods are not attempted.
- GitHub fallback binaries are checksum-verified and installed under a
  user-local `.moonup/bin` directory.
- PATH updates are opt-out through `NO_PATH_UPDATE`. POSIX updates the detected
  shell profile in the Pixi installer style; Windows updates both the user
  environment and the current PowerShell process.
- The extensionless script requires careful maintenance of the interpreter
  boundary; syntax changes must be checked with both `sh` and PowerShell.
