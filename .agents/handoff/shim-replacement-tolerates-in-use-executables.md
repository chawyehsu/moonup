# Handoff: Shim replacement that tolerates in-use executables

> **Status: IMPLEMENTED.** The `## Summary` and `## Decisions` sections record the agreed
> (final) design from the `grill-with-docs` session; the `## Implementation` section records
> what was built, the resolved open items, and verification results.

## Summary

`crate::utils::replace_exe` (Windows path) retires a shim to a constant `moon.exe.old`
name and deletes the previous `.old` first. A shim in use (a running `moon` command
holds the shim's own image; Windows permits renaming but not deleting a running image)
leaves a locked `moon.exe.old`; the next replacement dies at the removal step, and the
`moon.exe → .exe.old` rename then fails because the `.old` already exists and is locked.

Fix: (1) byte-compare guard — skip replacement when dest is byte-identical to source;
(2) Windows retire into `MOON_HOME/bin/.trash/` under a unique `pid`+counter name, sweep
best-effort after copy, migrate the legacy `.old`. Everything but the `copy` becomes
best-effort.

## Decisions (agreed, final)

Full rationale and rejected alternatives: `.agents/adr/0002-windows-shim-replacement-tolerates-in-use-executables.md`
Glossary: `CONTEXT.md` — terms **Shim**, **Retire**.

1. **Scope:** fix `replace_exe` only. Directory `swap()` in `src/toolchain/atomic.rs`
   keeps its fixed `.old` retired name (rare collision; name is load-bearing for
   `recover`/`sweep_staging`).
2. **Compare-and-skip guard, both platforms:** `files_identical(new, old)` — size
   fast-path (dest missing → replace; size mismatch → replace), then 64 KB streaming
   byte compare with early-exit. Dest open/read failure → treat as *different* (never
   skip on weak evidence); source read failure → propagate as a real error. On skip,
   still best-effort `set_permissions(0o755)` on Unix so skip == replaced. No hashing,
   no size-only verdict.
3. **Windows replace:** `create_dir_all(bin/.trash/)` (on failure, fall back to a
   sibling unique `moon.exe.old.<pid>.<n>` next to dest, still best-effort removed
   after); `rename(old → .trash/moon.exe.old.<pid>.<n>)` ignoring `NotFound` (other
   errors fatal); best-effort remove legacy sibling `old.with_extension("exe.old")`;
   `copy` fatal; post-copy best-effort `remove_dir_all(.trash)`.
4. **Uniqueness:** `std::process::id()` + static `AtomicU64` counter. Build names by
   string append — NOT `with_extension` (dot-in-extension mangles the stem).
5. **Unix path:** unchanged (remove + copy + chmod 0o755); trash machinery
   `#[cfg(target_os = "windows")]` only, matching current structure.
6. **Tests:** cross-platform behavior tests + Windows-gated in-use test using a real
   running image (child process holding a copy of the current test exe), NOT a
   synthetic share-mode handle (that cannot reproduce rename-ok/delete-denied).
   No `windows-sys` dependency.

## Implementation

- `src/utils.rs`:
  - `TRASH_DIR = ".trash"` (`:117`, Windows-only constant).
  - `files_identical` (`:124`, `pub(crate)`) + `read_up_to` (`:159`): size fast-path,
    then 64 KB streaming byte compare with early exit; dest metadata/open failure →
    `Ok(false)`, source failure propagates.
  - `retire_exe` (`:180`, Windows): `create_dir_all(bin/.trash/)`, retire as
    `moon.exe.old.<pid>.<n>` (string-append name, `std::process::id()` + static
    `AtomicU64`), sibling-unique fallback on setup failure, `NotFound`-only rename
    tolerated.
  - `replace_exe` (`:223`): normalize dest to `.exe` on Windows → guard (Unix chmods
    `0o755` on skip) → Windows: retire → remove legacy `old.with_extension("exe.old")`
    → copy (fatal) → best-effort `remove_dir_all(.trash)` + `remove_file(retired)`.
    Unix path unchanged.
- `tests/integration/replace_exe.rs` (new; registered in `tests/integration/mod.rs`):
  - `replace_exe_pours_new_content`, `replace_exe_pours_when_dest_missing` —
    cross-platform.
  - `replace_exe_skips_identical_content` — asserts the dest mtime is untouched and
    (Unix) the mode is normalized to `0o755`.
  - `replace_exe_hold_guard` (Windows) — copies the current test exe to the dest,
    spawns it as a child with `MOONUP_TEST_HOLD=1` + the test-name filter (child sleeps
    60 s), replaces while the image is running, asserts the retired copy lingers in
    `.trash`, then kills/reaps the child and asserts the next replacement sweeps `.trash`.

### Resolved open items

- `.trash` directory name constant: `TRASH_DIR = ".trash"` in `src/utils.rs`, Windows-only.
- `files_identical` visibility: `pub(crate)`; tested black-box via `replace_exe`.
- Hold-guard test: `replace_exe_hold_guard`, child sleeps 60 s only when
  `MOONUP_TEST_HOLD` is set, parent kills/reaps the child.
- Legacy-sweep target: `old.with_extension("exe.old")` (covers the pre-fix convention).

### Verification

- `cargo fmt --check` clean; `cargo clippy --all-targets` clean; `cargo test --test test`
  39 passed on Windows (hold-guard included). No `windows-sys` dependency added.

## Sensitive information

None. No keys, credentials, or PII were part of this session.
