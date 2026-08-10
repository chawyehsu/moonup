# Handoff: Incomplete stable-version selectors (issue #189)

## Status

Design confirmed. Implementation is the next task.

## Agreed contract

- Phase 1 changes `moonup install` only.
- Accepted incomplete forms are numeric prefixes with one, two, or three
  dot-separated components: `0`, `0.10`, `0.10.1`.
- No optional `v` prefix, prerelease syntax, general semver ranges, or nightly
  partial selectors.
- Full build-qualified versions remain exact.
- Three-part unqualified versions prefer an exact unqualified release; if none
  exists, they fall back to the newest matching `version+build` release.
- One- and two-part selectors choose the last matching release.
- Releases are filtered for host support before matching; the stable index is
  oldest-first and authoritative.
- `InstallRecipe` retains the requested selector as metadata and uses the
  concrete resolved version as its effective `spec` and filesystem identity.
- Log resolution at INFO only when the requested and resolved values differ.
- Keep the existing index-cache behavior.

## Implementation seams

1. Add a pure resolver/selector module with unit tests for grammar,
   precedence, ordering, host filtering, exact build matches, malformed input,
   and no-match behavior.
2. Integrate resolution into `build_installrecipe` for stable version specs.
3. Ensure recipe construction, atomic recovery, package download paths, and
   post-install linking use the concrete effective spec.
4. Retain the original selector in recipe metadata without creating selector
   directories or persistent selector state.
5. Update install help/user-facing documentation and snapshots as needed.

## Explicitly deferred

Partial selectors in `pin`, `default`, `run`, shim lookup, `update`, or `list`;
selector aliases; persistent selector metadata; nightly selector ranges; and
general semver range support.

See ADR `.agents/adr/0003-incomplete-stable-version-selectors.md` and the
glossary entries in `CONTEXT.md`.
