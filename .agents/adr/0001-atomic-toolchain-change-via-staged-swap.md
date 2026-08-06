# Atomic toolchain changes via staged directory swap

`populate_install` previously deleted the live toolchain directory in place and then extracted the new toolchain into the same location, so a failure at any point (a locked executable on Windows, or a killed process on Unix) left the live toolchain corrupted. We now assemble every toolchain in a staging directory under `toolchains/.staging/` and swap it into place with two same-volume renames (`live` → `.staging/live.old`, then `.staging/live.new` → `live`), retiring the previous directory best-effort afterwards. A failed operation leaves a discarded staging directory, never a damaged live toolchain. On Windows a running executable can be renamed but not deleted, so a retired `.old` that is still in use cannot be swept; it is shunted aside under a unique tombstone name (`.staging/<name>.old.<pid>.<n>`) so it never blocks the next swap, which removes it best-effort once its processes have exited.

## Considered Options

- **Process enumeration to name the locking process** — rejected: renaming a directory succeeds even when a child executable is in use, so the Access-Denied case is now rare and self-healing; the probing dependency and filesystem walk were not justified.
- **Symlink indirection** (`latest` → versioned dir, atomically repointed) — rejected: deep refactor of every path resolution, shim, `link_dirs`, and `installed_toolchains`, for a microsecond-wide risk.

## Consequences

- A tiny window exists between the two renames where the live name doesn't resolve; a concurrent shim invocation in that window would auto-trigger an install. Recovery heals the state on the next `install` or `update`.
- Staging, retired, and live directories are same-volume by construction (all under `toolchains/`), preserving rename atomicity on both Windows and Unix.
- A stale retired directory whose locking process is still running is shunted to a tombstone name rather than failing the swap; the leftover self-converges once its processes exit. This supersedes the "rename-of-a-directory-with-a-running-child-succeeds, so the Access-Denied case is rare and self-healing" assumption (which also informed the shim-retirement work): rename is forgiving, delete is not, so the deterministic `.old` slot must never be assumed free.
