# Windows shim replacement tolerates in-use executables

`replace_exe` previously retired a shim to a constant `moon.exe.old` name and removed the previous `.old` first, so a shim held open by a running command (Windows allows renaming but not deleting a running image) left a locked `.old` that made the next replacement fail outright. We now compare the source and destination bytes first and skip identical files entirely, and retire any shim that genuinely needs replacing into the hidden `MOON_HOME/bin/.trash/` directory under a unique `pid`-counter suffix, sweeping the trash best-effort afterwards. A locked leftover simply waits for its process to exit and is swept by a later run, so a replacement in progress can never be blocked by a stale retired file.

## Considered Options

- **Constant `.old` suffix** (status quo) — rejected: a locked `.old` from the previous run is a hard failure at the next run's removal step.
- **Skip-and-warn when in use** — rejected: leaves a stale shim running and, for `selfupdate`, a new `moonup` binary that never takes effect.
- **Delay deletion until reboot** (`MoveFileEx(MOVEFILE_DELAY_UNTIL_REBOOT)`) — rejected: requires elevation, leaves `PendingFileRenameOperations` registry entries, and is disproportionate for self-healing leftovers.
- **Byte-compare with a hash** — rejected: direct streaming compare stops at the first differing byte and is cheaper than hashing both files; there is no signature cache to exploit.
- **Synthetic file lock in tests via a Windows API share-mode handle** — rejected: a share-mode handle locks too much (blocks the rename, which reality permits) or too little (no lock); only a real running image reproduces the rename-ok/delete-denied behavior the fix depends on.

## Consequences

- Identical shims are left untouched: repeated toolchain installs stop churning shims entirely, and a shim is only touched when `moonup` itself changed. On Unix the skip still normalizes permissions to `0o755`, so skipping is behaviorally identical to replacing.
- Retired files accumulate in `.trash/` only while their processes run, then disappear on the next replacement's best-effort sweep; the directory self-converges and is namespaced so the sweep can never touch a live shim. When `.trash/` cannot be created, the retire falls back to a unique sibling `moon.exe.old.<pid>.<n>` name that is still swept best-effort.
- The legacy `moon.exe.old` convention is swept on each replacement, migrating pre-fix installs.
- The copy step, a rename failing for a reason other than `NotFound`, and an unreadable *source* in the byte-compare guard are the only fatal steps; a missing or unreadable *destination* is treated as *different*, never a reason to skip. Everything else — trash setup, the legacy sweep, and the post-copy trash sweep — is best-effort.
