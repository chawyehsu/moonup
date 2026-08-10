# Install incomplete stable-version selectors as concrete releases

`moonup install` accepts an incomplete numeric stable-version selector such as
`0` or `0.10`, and resolves it to a concrete release from the stable channel
index. A three-part selector such as `0.10.1` first prefers an exact
unqualified release; if none exists, it selects the newest matching
build-qualified release such as `0.10.1+abcdef123`. Full build-qualified
versions remain exact.

Resolution considers only releases supported by the current host. The stable
channel index is authoritative and is ordered oldest-first, so the newest
matching release is the last matching entry. Existing index-cache behavior is
unchanged.

The original selector is retained as recipe metadata for diagnostics, while
the resolved concrete version becomes the effective toolchain identity and
installation directory. A partial selector therefore never creates a
non-reproducible selector-named directory.

## Considered Options

- **Install under the selector name** — rejected: the selector can resolve to a
  different release as the index changes, and selector-named directories would
  duplicate concrete installations.
- **Sort releases independently** — rejected: the distribution index defines
  release order, including the otherwise unordered build metadata suffix.
- **Support general semver ranges or nightly prefixes** — deferred: phase 1 is
  intentionally limited to numeric stable selectors and install-time
  resolution.
- **Resolve partial selectors in every command** — deferred: the semantics of
  pinning, default selection, running, updating, and aliases require a separate
  design.

## Consequences

- `install 0`, `install 0.10`, and `install 0.10.1` can select the newest
  compatible stable release without requiring a build hash.
- Installing a selector and then its concrete result converges on one
  installation directory.
- The selected concrete version should be logged at INFO level when it differs
  from the requested selector.
- Phase 1 does not persist the selector or make partial selectors meaningful to
  `pin`, `default`, `run`, shim lookup, `update`, or `list`.
- The resolver should be a pure, independently tested seam over parsed stable
  releases and host support.
