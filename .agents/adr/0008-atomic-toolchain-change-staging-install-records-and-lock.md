# Atomic toolchain changes: staging, install records, and the toolchain lock

`populate_install` previously deleted the live toolchain directory in place and
then extracted the new toolchain into the same location, so a failure at any
point (a locked executable on Windows, or a killed process on Unix) left the
live toolchain corrupted. Every toolchain change now assembles the new
toolchain in a staging directory under `toolchains/.staging/` and swaps it into
place with two same-volume renames — `live` → a uniquely named retired
directory, then staging → `live` — retiring the previous directory best-effort
afterwards, and restoring it to the live name if the promotion itself fails.
A failed operation leaves a discarded staging directory, never a damaged live
toolchain.

Completeness is certified by the install record itself: the JSON `version`
file inside every toolchain directory (live or staged) carries the identity —
channel, release key, floating-ness — and, while finalization (pouring shims,
building the core library, linking directories) is still outstanding, a
`finalization` object whose presence marks a staging as promotable and a
promoted install as awaiting its post-install steps. `acknowledge` rewrites
the record without the object once finalization succeeds, and the record is
written via temp-file + rename so a crash never leaves a truncated record. A
staging whose record carries no payload — a partial extraction, or a leftover
from an older moonup — is never promoted; the next install sweeps it and
re-downloads.

Retired directories are parked under unique `<name>.old.<pid>.<n>` names (the
same convention as shim retirement), so a leftover that Windows cannot delete
yet — a process still holds an executable from the previous toolchain, which
Windows lets us rename but not delete — never blocks the next swap; later
swaps sweep such leftovers best-effort once their processes have exited.

All toolchain mutations (`install`, `update`, `uninstall`, and the recovery
they run) hold an exclusive advisory lock on `toolchains/.lock` for their
duration, acquired after any interactive selection. A shim-triggered
auto-install therefore waits for a running `update` instead of racing it
around the same staging paths, and the lock needs no cleanup because advisory
locks die with the holding process.

## Considered Options

- **Symlink indirection** (`latest` → versioned dir, atomically repointed) —
  rejected: deep refactor of every path resolution, shim, `link_dirs`, and
  `installed_toolchains`, for a microsecond-wide risk.
- **Process enumeration to name the locking process** — rejected: renaming a
  directory succeeds even when a child executable is in use, so the
  Access-Denied case is rare and self-healing; the probing dependency and
  filesystem walk were not justified.
- **Sibling completeness marker** (`.staging/<name>.complete`, as previously
  shipped) — rejected: a certificate stored outside the artifact it certifies
  can diverge from it (marker without staging, staging without marker,
  unreadable marker), and each divergence demanded its own recovery branch
  and serde compatibility rules. Folding the certificate into the record that
  travels with the directory leaves exactly one file to trust, and the old
  `StagedRelease`/`into_recipe` machinery — including the stable-selector
  dance, which never matched a real crash state — is deleted with it.
- **Deterministic retired slot** (`.staging/<name>.old`, as previously
  shipped) — rejected: the fixed slot is what turned a locked leftover into a
  blocker, requiring sweep-then-shunt-then-check, unique tombstones, and a
  user-facing "previous toolchain is still in use" retry error for a collision
  that unique naming never creates.
- **Fail-fast lock** (error when another moonup operation is running) —
  rejected: a shim-triggered auto-install would fail the user's `moon build`
  during a concurrent update; blocking turns the race into a wait, after which
  the shim finds the toolchain present and skips the install.

## Consequences

- On-disk state per toolchain is three artifact kinds: the live directory, the
  staging directory (complete iff it holds a matching install record with a
  finalization payload), and uniquely named retired leftovers, swept
  best-effort by a glob that also removes legacy artifacts (`<name>.old*` and
  the legacy `<name>.complete` markers). The literal dot after the install
  name keeps a spec whose name prefixes another (e.g. `nightly` vs
  `nightly-2025-01-01`) from sweeping the other's leftovers.
- A tiny window between the two renames remains where the live name does not
  resolve. Concurrent moonup processes now wait on the lock instead of
  auto-triggering an install into that window; a non-moonup reader in the
  window simply sees no toolchain until the swap lands, and the next
  operation's recovery heals any state it could observe.
- `acknowledge` is best-effort: a failed rewrite leaves the payload in place,
  and the next install or update re-runs the post-install steps, which are
  idempotent.
- Crash states left by versions that wrote the separate `.complete` marker are
  swept and re-downloaded rather than promoted. This costs one re-download in
  a rare, self-healing case and deletes the serde compatibility branches.
- The record schema grows only inside the `finalization` object, so future
  post-install inputs need no new file, marker, or compatibility rule.
- Because the swap's restore path remembers only the retired directory it
  created this swap, a staging lost outside a swap (crash between the two
  renames plus manual deletion of the staging) is healed by re-downloading
  rather than by restoring the leftover; the leftover stays intact until the
  next successful swap sweeps it.
