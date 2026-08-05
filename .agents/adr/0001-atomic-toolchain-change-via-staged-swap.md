# Atomic toolchain changes via staged directory swap

`populate_install` previously deleted the live toolchain directory in place and then extracted the new toolchain into the same location, so a failure at any point (a locked executable on Windows, or a killed process on Unix) left the live toolchain corrupted. We now assemble every toolchain in a staging directory under `toolchains/.staging/` and swap it into place with two same-volume renames (`live` → `.staging/live.old`, then `.staging/live.new` → `live`), deleting the retired directory best-effort afterwards. A failed operation leaves a discarded staging directory, never a damaged live toolchain.

## Considered Options

- **Process enumeration to name the locking process** — rejected: renaming a directory succeeds even when a child executable is in use, so the Access-Denied case is now rare and self-healing; the probing dependency and filesystem walk were not justified.
- **Symlink indirection** (`latest` → versioned dir, atomically repointed) — rejected: deep refactor of every path resolution, shim, `link_dirs`, and `installed_toolchains`, for a microsecond-wide risk.

## Consequences

- A tiny window exists between the two renames where the live name doesn't resolve; a concurrent shim invocation in that window would auto-trigger an install. Recovery heals the state on the next `install` or `update`.
- Staging, retired, and live directories are same-volume by construction (all under `toolchains/`), preserving rename atomicity on both Windows and Unix.
