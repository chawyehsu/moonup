# Extract concrete release ordering into a channel-aware module

Extract comparison of concrete release identities into a pure reusable module. The module consumes an already oldest-to-newest channel release sequence, validates every version, requires both operands to exactly match channel entries, and returns `Result<Ordering, CompareError>`; it does not fetch indexes, read caches, or sort the input. Channel order remains authoritative because numeric comparison cannot represent build-qualified release precedence.

The reusable API compares `A` relative to `B`; callers apply `>`, `>=`, `<`, or `<=` to the returned `Ordering`. Both operands must be fully qualified concrete identities, including `+build` metadata where present. User-provided partial version selectors remain the responsibility of `toolchain::version` and are not part of this module.

## Considered options

- **Numeric sorting of all versions** — rejected: it loses channel-defined ordering and cannot distinguish build metadata.
- **Defensive sorting inside the module** — rejected: the channel index already guarantees order, and sorting would replace authoritative ordering with inferred ordering.
- **Base-version thresholds and operator-aware anchors** — rejected: they introduce ambiguity when multiple builds share one base version and are unnecessary for internally provided concrete identities.
- **Silent filtering of malformed releases** — deferred: filtering can change channel semantics; strict validation is safer until a concrete data-quality requirement exists.
- **Operator-bearing or boolean APIs** — rejected: `Result<Ordering, CompareError>` is mechanical and lets callers express any relational predicate without duplicating comparison logic.

## Consequences

- Future channel-gated features can compare internally provided concrete releases without accepting or interpreting user syntax.
- Moonup-specific cache fallback and installed-toolchain display ordering remain adapters around the pure module.
- The first implementation should migrate `NumericVersion` and add tests for concrete releases with build metadata, ordering in both directions, equal identities, missing operands, malformed channel entries, and empty sequences.

## Implementation plan

1. Define a small public `Version` parser/value type and `compare(channel, a, b) -> Result<Ordering, CompareError>` API independent of `dist_server::Release`.
2. Validate the complete channel sequence and require exact membership for both operands. Preserve the supplied oldest-to-newest order; never sort it.
3. Replace the private numeric-version implementation used by installed-toolchain fallback with the shared value type, retaining existing cache and display behavior.
4. Keep `toolchain::version::resolve_stable_selector` separate for partial user selectors; do not route selector parsing through the reusable comparison API.
5. Add focused unit tests for exact build-qualified identities, duplicate base versions, both comparison directions, equality, missing/malformed data, and empty channels.
6. Use the comparison API in the future feature gate with an internally configured fully-qualified threshold, propagating comparison errors so the feature is unavailable when the channel cannot establish a safe ordering.
