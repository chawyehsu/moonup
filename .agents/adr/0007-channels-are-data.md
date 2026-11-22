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

## Amendment: channel-qualified pinned installs

Bare version pins made "the same version in two channels" unrepresentable: a
pinned install's identity was the version string alone, so `0.11.0` from `latest`
and `0.11.0` from `lts` would share one install directory. Pinned installs are
therefore channel-qualified:

- A pinned spec is a `(channel, key)` pair, the key being the release's build
  date for dated releases, otherwise its version. Input accepts
  `{channel}<sep>{key}` with `<sep>` one of `/`, `-`, `@` (`lts/0.11.0`,
  `lts-0.11.0`, `lts@0.11.0`); channel
  names conventionally contain no separator (moonup logs at debug level if one
  does). The canonical string form is bare `{key}` for `latest`-channel pins and
  `{channel}-{key}` otherwise (so legacy `nightly-<date>` pins keep their
  familiar display form).
- **`latest` is a must-have, fixed-name default channel** of any dist server
  (amending the "derive default from index" rejection above: the default channel
  is the fixed name `latest`, not index position). Bare versions are pins in
  `latest`. This keeps spec parsing fully offline.
- Install directories stay flat: floating channels at `toolchains/{channel}`,
  `latest`-channel pins at `toolchains/{key}` (unchanged), other channels' pins
  at `toolchains/{channel}-{key}` (which reproduces the existing
  `nightly-2026-01-01` dir naming). No migration or renaming of existing
  installs. Nesting pins under their channel's floating directory was rejected:
  updating a floating channel renames its directory during the atomic swap,
  which would sweep nested installs away.
- The `version` stub becomes a JSON metadata file (same filename) written for
  every install: `{"channel": "...", "key": "..."}`. Floating-ness is derived
  (install dir name equals the channel name), not stored. Legacy plain-text
  `version` files parse as floating and are left as-is on read paths — they are
  rewritten in the JSON format whenever the toolchain is reinstalled or updated.
  Legacy pinned dirs (no stub) keep the name heuristics.
- The completeness marker records the spec's channel so crash recovery works
  offline for qualified pins; old markers stay readable.
- `moonup list` orders a pinned install by its key's position in its own
  channel's cached channel index, falling back to numeric ordering.

## Consequences

- Spec interpretation moves to resolve time: a user string matching an index
  channel name is a channel, otherwise it is parsed as a qualified or bare
  version pin; unresolvable input errors with the list of available channels.
  Bare version pins always search the `latest` channel.
- `moonup list` ranks channels by their position in the cached main index
  (best-effort offline); unranked channels sort last.
- `bleeding` is removed: install/update error with available channels, offline
  uninstall of existing on-disk installs still works, stale `moonbit-version`
  pins error clearly.
- Old component-index cache directories (`downloads/bleeding`,
  `downloads/latest/{version}`, `downloads/nightly/{date}`) are orphaned; they
  expire by timestamp and never matter beyond the 1-hour cache window.

## Amendment: channel validation and unambiguous pin syntax

This amendment supersedes the earlier amendment's separator and canonical-form
decisions.

Channel names are data, but they must still be addressable by Moonup. A channel
name must be non-empty and contain only lowercase ASCII letters, digits, and
hyphens. Invalid channel entries from the main index are ignored with a warning;
invalid user-provided channel names are rejected.

Qualified pin input uses `/` or `@`, with `@` as the canonical spelling and the
form shown by `list` and `pin`. `-` is no longer a general separator, since it is
valid within a channel name. The exact legacy form `nightly-YYYY-MM-DD` remains
accepted everywhere a toolchain spec is consumed, provided the suffix is a real
calendar date. That shape is reserved for the legacy nightly pin. Latest-channel
pins remain bare release keys, including keys with build metadata.

Reading a legacy dashed pin file does not rewrite it. An explicit `moonup pin`
write uses the canonical spelling. Install-directory names remain unchanged:
latest pins use their bare key, and other-channel pins use
`{channel}-{key}`. The per-install metadata records channel and key separately
and takes precedence over directory-name inference. Metadata-free legacy
directory inference retains only the nightly-date compatibility form, alongside
the existing latest-key recognition for numeric and `+` build-metadata names.
Future prerelease keys containing hyphens rely on per-install metadata, which
every new install writes before promotion into the live toolchains directory.

The toolchains directory remains the source of truth for installed-toolchain
inventory. A global manifest is not introduced: it would duplicate that state
and could drift after manual directory changes or interrupted install/update/
uninstall operations. A future short-lived, derived I/O cache may be considered
if inventory scans prove costly; it must not become an independent source of
truth.
