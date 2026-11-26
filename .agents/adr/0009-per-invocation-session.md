# Per-invocation session for path resolution and index caching

Every operation in moonup re-derives paths from the moonup home directory at
its point of use — `moonup_home()` is a free function re-evaluated at each of
roughly twenty call sites — and re-reads the cached index files from disk each
time they are needed. A single invocation touches the same files repeatedly:
`install` reads the main index twice and a channel index twice before
extracting anything; `update` reads the main index and the channel indexes
once per floating channel to rank installed toolchains, then re-reads them
again inside each channel's recipe, so a warm on-disk cache is still parsed
N+1 times per invocation. Worse, the release-ordering module reads those same
index files through a separate synchronous path that bypasses the freshness
check entirely, so ranking can consult stale disk content that the
asynchronous code path has already replaced.

Moonup therefore introduces a **session**: a context created once per CLI
invocation in `cli::start()`, passed as `&Session` to every subcommand's
`execute`, that resolves all home-derived paths up front and serves index
metadata from an in-memory cache for the remainder of the invocation. The
session also owns the distribution-server HTTP client: today a client is
rebuilt for each download, so the several files a single `install` or
`update` fetches pay a fresh TLS handshake apiece and share no connection
pool; the session constructs one client lazily and reuses it across every
request the invocation makes. The session is layered. A std-only `Layout`
resolves both home directories once at construction and derives every
location from them — toolchains root, downloads, the three index locations,
the default file, the lock file — as pure computation off the stored roots;
it is the single source of truth for path construction in the codebase. The
shim binary constructs a `Layout` in its `main` and threads it through its
synchronous flow, preserving its hard constraint
of staying a tiny, fast, synchronous proxy that links none of the
session/caching machinery. The full `Session` lives above it: the layout,
the index cache, and the shared HTTP client.

The cache covers exactly the three index readers — main index, channel index,
component index — stored parsed behind std synchronization primitives
(`Mutex`/`OnceLock`; no `.await` occurs while a lock is held, and values
crossing into `spawn_blocking` closures are owned clones). The
release-ordering module routes through the same cache instead of its private
disk reads: when the invocation has already loaded an index, ordering reads
the session's copy; otherwise it keeps its offline, best-effort disk read —
which is deliberately never stored back, so a stale disk copy can neither
poison the session's view nor suppress a freshness re-fetch. Consistency
needs no invalidation API: whenever the index layer fetches fresh data from
the distribution server and rewrites the on-disk cache files, it also updates
the session's in-memory entry, so the cache acts as the source of truth for
the invocation's lifetime — the same write-through shape the index layer
already applies to disk. Any file not re-read within a single invocation is
explicitly out of scope: install records, download archives, and shim
binaries keep their current read-on-demand behavior.

## Considered Options

- **Process-global statics** (home resolved once via a `OnceCell`, a global
  cache keyed by path) — rejected: tests override `MOONUP_HOME` per test
  through the process environment; a global initialized on first access can
  never be reset, so the entire existing test approach would break.
- **`&mut Session` threaded through the call tree**, with mutability coming
  from the caller — rejected: subcommands hand work to `spawn_blocking`
  closures that must own their data (`'static`), so a `&mut` borrow cannot
  reach the places that need session data; interior mutability behind shared
  references fits the async runtime instead.
- **RefCell-style interior mutability** — rejected: it is not `Sync`, and
  moonup runs on the multi-threaded tokio runtime where the session is
  referenced across tasks; std `Mutex`/`OnceLock` provide the same lazy
  single-initialization guarantees safely.
- **Caching all file reads** — rejected: duplicated reads were measured to
  exist only in the index layer; install records, the default file, and pin
  files are read one or two times per invocation, so a broader cache adds
  surface without a measured win. Re-reading from disk remains perfectly
  viable wherever no duplication exists, and stays the fallback for any file
  whose caching would demand real invalidation machinery.
- **Leaving the ordering module on its private disk reads** — rejected: it
  silently ranks against stale content the rest of the invocation has already
  replaced; routing it through the shared cache costs nothing extra and
  removes the second, divergent read path.
- **Keeping per-download client construction** — rejected: the requests of
  one invocation are known up front and share the same hosts, so a
  session-owned client is no more complex than the current rebuild-per-use
  helper, while reusing pooled connections instead of paying a fresh
  handshake for every downloaded file.

## Consequences

- Every subcommand signature grows a `session: &Session` parameter, and the
  index readers move from free functions to session accessors. This is the
  bulk of the change's mechanical cost, paid once.
- `update` with a warm on-disk cache no longer re-parses the same JSON per
  channel, and `install` reads each index file from disk at most once.
- Downloads within one invocation share pooled connections: multi-component
  installs and multi-channel updates stop paying a TLS handshake per file.
- Release ordering serves the session's cached index whenever the invocation
  has already loaded it, while its cold-path disk read stays offline and
  best-effort exactly as before — no network is introduced into `list` or
  ranking, and a stale disk copy cannot leak into the session's view.
- The shim binary is unchanged in behavior and size: it constructs a
  `Layout` (which resolves the homes once, exactly as a session would) and
  links none of the session/caching machinery; a shim invocation continues
  to resolve its toolchain per process without index access.
- Cross-invocation staleness is unchanged by design: each invocation starts
  from disk state (subject to the existing one-hour on-disk freshness rule)
  and never observes another invocation's in-memory state.
- This record intentionally quantifies no gains. The implementation should be
  accompanied by a short report measuring what the session actually bought —
  duplicate reads eliminated, requests per invocation, wall-clock deltas on
  representative `install`/`update` runs — so the decision's effectiveness is
  confirmed with data rather than asserted here.
