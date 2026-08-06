# Handoff: Atomic toolchain change (issue #195)

> **Status: FINISHED — DONE.** Do not re-handle. Implementation landed in
> PR <https://github.com/chawyehsu/moonup/pull/196> (closes #195); review
> feedback was addressed. See "Completion" below for where things landed.

## Summary

moonup has a non-atomic toolchain replacement. `populate_install` deletes the live
toolchain dir in place then extracts the new one, so a mid-operation failure (locked
exe on Windows, Ctrl-C kill on Unix) corrupts the live toolchain. In this session we
grilled through the design and reached shared understanding; the next session
implements it.

~~**Next session's task:** implement the design below. Design decisions are final — do
not re-litigate; implementation details flagged as "open" still need decisions.~~

## Completion

Implemented and shipped (PR #196). The design decisions below were followed;
the open items were resolved during implementation as follows:

- Module: `src/toolchain/atomic.rs` (named `atomic`), holding the single shared
  `swap()` plus `recover()`, `acknowledge()`, `staging_dir_for`,
  `completeness_marker_for`, `retired_dir_for`, `is_complete`, `staged_matches`,
  `sweep_staging`, and the persisted `StagedRelease` metadata.
- Completeness marker: a dedicated JSON marker file `.staging/<name>.complete`
  (uniform across all specs; stores release identity: `version`, `date`,
  `bundle_source_dir`). `swap()` retains it as a pending-finalization record;
  callers remove it via `acknowledge()` after `post_install`/`link_dirs`.
- `.staging` filtering: `installed_toolchains()` skips dot-prefixed entries.
- `uninstall` sweeps `.staging` leftovers for the removed toolchain.
- The old `empty_dir`-based in-place delete in `package.rs` was removed;
  `populate_install` assembles into `.staging/<name>.new`, writes the marker,
  then `swap()`s. Staging is only reused when `staged_matches` the current recipe.
- Recovery runs before any network in `install`/`update`; `recover()` promotes a
  complete staging when the live dir is missing and returns `StagedRelease` so
  callers finalize (shims/links/bundle) with the persisted metadata.

Tests: `tests/integration/atomic.rs` and `tests/e2e/atomic.rs`
(feature-gated `test-liveinstall`). All build/fmt/clippy clean; suites pass.

## Decisions (agreed, final)

Full rationale and rejected alternatives are in the ADR; glossary in CONTEXT.md.
Read both before implementing:

- ADR: `.agents/adr/0001-atomic-toolchain-change-via-staged-swap.md`
- Glossary: `CONTEXT.md` (root) — terms: Toolchain, Toolchain spec, Atomic toolchain change, Staging directory, Populate
- Issue: <https://github.com/chawyehsu/moonup/issues/195>

1. **Strategy: stage + swap, cross-platform uniform.** Assemble new toolchain in
   `toolchains/.staging/<name>.new`, then two same-volume renames:
   `toolchains/<name>` → `.staging/<name>.old`, then `.staging/<name>.new` →
   `toolchains/<name>`. Retire old dir best-effort. Never delete live dir in place.
   Old toolchain stays fully intact until the commit rename.
2. **Swap failure contract:** fail cleanly, keep `.staging/<name>.new` for retry.
   Error message: "the previous toolchain is still in use, close programs using it and retry".
   No per-file/process probing — this was explicitly dropped (renaming a dir succeeds
   even when a child exe is in use; failure only occurs when the dir handle itself is
   held, rare and self-healing).
3. **Recovery:** a `recover()` (own module, see Open Items) called at the top of BOTH
   `install::execute` and `update::execute`, BEFORE `build_installrecipe` (network).
   If live dir missing but complete staging exists → promote staging, return early
   (silent self-heal of the shim auto-install). `uninstall` sweeps `.staging`
   leftovers for the removed toolchain.
4. **Naming:** nested hidden dot-dir `.staging/` (not visible siblings). Must be
   filtered out of `installed_toolchains()`. Centralized `.staging` enables a future
   `moonup clean` command.
5. **Completeness marker:** staging must not be promoted if extraction was killed
   midway. Reuse the `version` stub (written pre-swap, currently written at the end
   of `populate_install`) OR a dedicated marker — open decision.

## Facts from code exploration (verify before relying)

- `src/fs.rs:34` `empty_dir` wraps `remove_dir_all::remove_dir_contents`, returns
  path-less `io::Error`. `src/fs.rs:18` `remove_dir_all` wrapper.
- `src/toolchain/package.rs:71` is the in-place `empty_dir` call to replace.
  `populate_install` also writes the `version` stub at package.rs:209-222 and cleans
  up invalid installs at package.rs:200.
- `src/toolchain/mod.rs:172` `InstalledToolchain::from_path` lowercases EVERY dir
  under `toolchains/` and parses it as a spec — `.staging` must be excluded here.
- `src/cli/install.rs:112` and `src/cli/update.rs:50` are the two `populate_install`
  callers. `update.rs:28-33` reads `toolchains/<name>/version` BEFORE building the
  recipe; a crash-state (missing version stub) currently prints "not installed" and
  skips — recovery must gate before this.
- `src/bin/moonup-shim.rs:68-86` auto-runs `moonup install <spec>` when
  `toolchains/<spec>` doesn't exist — the recovery vector for crash states.
- `src/runner.rs:13-16` resolves exes from `toolchains/<spec>/bin`.
- `src/cli/uninstall.rs:124` `remove_dir_all` on live dir.
- Windows rename: `std::fs::rename` requires same volume; nested `.staging` under
  `toolchains/` guarantees it. Dot-prefix dirs have no special filesystem meaning on
  Windows NTFS or POSIX rename.
- Toolchain mutators are exactly: `install`, `update`, `uninstall`. `run`/`which`/
  `list`/`default`/`pin`/`completions` are read-only. `selfupdate` is its own concern.
- Cargo: `remove_dir_all = "1.0.0"`, `miette` for diagnostics, `junction` crate on
  Windows. No `windows-sys`/`sysinfo` direct deps (and none should be added — probing dropped).

## Open items (need decisions during implementation)

- Module name/placement: `recovery.rs` vs `atomic.rs`, and whether `swap()` lives
  there too (agreed: single shared `swap()` used by both `populate_install` and
  `recover()`; do NOT duplicate the three-path rules).
- Completeness marker: reuse `version` stub vs dedicated marker file.
- Exact `.staging` filtering mechanism in `installed_toolchains()`.
- `uninstall` `.staging` sweep details.
- Whether existing `empty_dir`-based error wrapping at package.rs:73-74 is removed
  entirely with the in-place path.

## Repo conventions / workflow

- Language: Rust, edition 2024, async (tokio), miette diagnostics.
- Never leave a replaced toolchain corrupted (CONTEXT.md).
- Make changes small/atomic; commit messages `<topic>: <description>`; use `jujutsu`
  skill for change management (per AGENTS.md).
- No `.agents/adr/` existed before this session; ADR was created now.

## Suggested skills

- `jujutsu` — manage the change with stacked/atomic commits per repo workflow.
- `codebase-design` — the `swap()`/`recover()` seam between `populate_install` and
  recovery is a natural deepening opportunity; use its vocabulary when shaping the
  new module's interface.
- `domain-modeling` — the glossary already exists; use only if new terms crystallise.
- `grill-with-docs` — only if a new ADR-worthy decision emerges (e.g. completeness
  marker choice is likely too small; skip unless a real trade-off appears).

## Sensitive information

None. No keys, credentials, or PII were part of this session. Do not fabricate credentials.
