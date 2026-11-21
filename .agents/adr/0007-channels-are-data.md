# Channels are data, not code

Moonup previously hardcoded the set of dist server channels (`latest`, `nightly`,
`bleeding`) as enum variants, so any channel added to or removed from the remote
server required a source change — `bleeding`'s deprecation being the motivating
case. We decided Moonup becomes fully channel-agnostic: the main index is the
single source of truth for which channels exist, `ToolchainSpec` carries channel
names as data (`Channel(String)` / `Version(String)`), and no code matches on
specific channel names.

This freezes a dist-server URL contract: component indexes are addressed as
`/{channel}/{release-key}/{filename}` where `release-key` is the release's build
date when present, otherwise its version, and component archives are addressed
as `/download/{tag}/{filename}` where the tag is `{channel}-{date}` for dated
releases and `v{version}` for versioned releases. This reproduces the old
`latest` and `nightly` layouts exactly; the deprecated `bleeding` channel's
unkeyed layout (`/bleeding/{filename}`) was the exception that forced permanent
channel knowledge, and its artifacts were migrated server-side to the
generalized layout.

## Considered Options

- **Keep known-channel enums, drop `bleeding` only** — rejected: still couples
  Moonup releases to server-side channel changes, the original problem.
- **Extend the index schema to advertise per-channel metadata (kind, layout)** —
  rejected: the generalized layout already covers every real channel without a
  schema bump; the `date`-vs-`version` release key distinction falls out of the
  release data itself.
- **Derive the default channel from the index** — rejected: the shim's
  default-toolchain fallback must work without network; the literal `latest`
  remains the conventional default.

## Consequences

- Spec interpretation moves to resolve time: a user string matching an index
  channel name is a channel, otherwise a version selector; unresolvable input
  errors with the list of available channels. Bare version selectors always
  search the `latest` channel; other channels are floating-only.
- `moonup list` ranks channels by their position in the cached main index
  (best-effort offline); unranked channels sort last.
- Shell completions resolve channel names from the cached index only — never a
  network call.
- `bleeding` is removed: install/update error with available channels, offline
  uninstall of existing on-disk installs still works, stale `moonbit-version`
  pins error clearly.
- Old component-index cache directories (`downloads/bleeding`,
  `downloads/latest/{version}`, `downloads/nightly/{date}`) are orphaned; they
  expire by timestamp and never matter beyond the 1-hour cache window.
