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
crossing into `spawn_blocking` closures are owned clones). Each entry is
bound to the on-disk `.lock` timestamp of the file it came from — the same
oracle the disk cache uses — which lets two read modes share one cache
safely. Fresh reads (`read_index`, `read_channel_index`,
`read_component_index`) serve a cached entry only while its timestamp still
matches the file and passes the freshness window; otherwise they consult
disk and fetch when stale, writing back to both disk and memory. Offline
reads (`read_index_offline`, `read_channel_index_offline`) serve memory or
disk, freshness ignored, and never perform a network request — these are
what the release-ordering module uses, so its reads join the cache instead
of living on a private disk path, and read-only commands never grow a
network dependency. Because "fresh" is only ever claimed when the timestamp
that produced an entry still matches the file, ordering-populated entries
can be reused without serving stale data as fresh. Consistency
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
- **Letting ordering use the freshness-enforcing reads** (awaiting
  `read_index`/`read_channel_index` directly) — rejected: freshness means
  fetch-on-stale, so `list`, `pin`, `default`, and `uninstall` would each
  grow a network round-trip whenever the on-disk cache is older than an hour
  — the common case — and would fail offline exactly when the stale disk
  copy is most valuable. Ordering's output is display-only, so freshness is
  not worth a network dependency; the dedicated offline mode serves that
  contract.
- **A mode flag on the read methods** — rejected: freshness is the caller's
  identity, not a per-call knob; a boolean at twenty call sites is opaque
  and easy to get wrong, where two named methods make each caller's
  contract self-documenting.
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
- Release ordering reads through the session's offline mode — memory or
  disk, never the network — so `list`, `pin`, `default`, and `uninstall`
  gain cache participation without gaining a single network request.
  Ordering still ranks against local data, stale when the on-disk cache is
  stale; since ordering only affects display order, this is accepted.
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
