# Moonup

Moonup is a toolchain manager for the MoonBit language. It installs, updates, and
selects MoonBit toolchains published on a distribution server, and shims `moon`
(and friends) to the active toolchain. Operations that replace an installed
toolchain must never leave it corrupted.

## Language

### Toolchain selection

**Toolchain**:
A named, self-contained installation of the MoonBit compiler and its components,
stored under `MOONUP_HOME/toolchains/<name>`.
_Avoid_: Install, build, distribution

**Toolchain spec**:
The user-facing install selector: either a channel name (floating: "newest of
that channel") or a pinned version.
_Avoid_: toolchain reference, spec string

**Channel**:
A named distribution line advertised by the dist server's main index (e.g. `latest`,
`nightly`). A channel has no fixed identity in Moonup: whatever names the index
lists are the channels that exist. Moonup attaches no semantics to any particular
name.
_Avoid_: branch, track

**Pinned version**:
A concrete release identified within its channel, written `<channel-prefix>-<date>`
for dated releases (e.g. `nightly-2026-11-19`) or as a plain compiler version for
versioned releases. The `nightly-` prefix is a pinned-format convention, not
per-channel machinery.
_Avoid_: dated spec, versioned channel

**Stable selector**:
A numeric, build-unqualified version string (1–3 dot-separated parts, e.g. `0.10`,
`0.10.1`) that resolves to the newest matching release in the `latest` channel.
Bare version selectors always search the `latest` channel; a selector is not
itself an installed toolchain identity.
_Avoid_: Version selector, partial version, version prefix

**Resolved release**:
The concrete stable release selected from the ordered stable release index for a
stable selector. The resolved release, rather than the selector, is the
toolchain's installation identity.

**Default toolchain**:
The toolchain used when nothing is pinned; stored in `$MOONUP_HOME/default`. When
unset, the literal name `latest` is the conventional fallback.
_Avoid_: global toolchain

### Distribution server artifacts

**Main index**:
The top-level document (`index.json`) listing every available channel with its
newest release. The single source of truth for which channels exist.
_Avoid_: root index, dist index

**Channel index**:
The per-channel document (`channel-{name}.json`) listing that channel's releases.
_Avoid_: release list

**Component index**:
The per-release, per-host document naming the downloadable components and their
checksums, addressed as `/{channel}/{release-key}/{host-target}.json`. The
component archives themselves live under `/download/{tag}/`, where the tag is
`{channel}-{date}` for dated releases and `v{version}` for versioned releases.

**Release key**:
The path segment identifying a release within a channel: the release's build date
when it has one, otherwise its version.

**Release**:
One publishable toolchain build within a channel, carrying its version, optional
build date, and supported host targets.
_Avoid_: build artifact, entry

### Installation and atomicity

**Atomic toolchain change**:
An operation that replaces an installed toolchain such that the currently
installed toolchain remains fully usable at every instant, even if the operation
fails or the process is killed mid-way.
_Avoid_: Safe delete, clean replace

**Staging directory**:
Where a new toolchain is fully assembled and verified before it is swapped into
the live install location. A failed install leaves a discarded staging directory,
never a corrupted live toolchain.

**Populate**:
The act of assembling a toolchain into its install location: downloading
components, extracting them, and making the toolchain live.
_Avoid_: Install in place

**Shim**:
A wrapper executable in `MOON_HOME/bin/` (and `bin/internal/`) that forwards a
toolchain command (e.g. `moon`) to the active toolchain's real binary via moonup.
It is a copy of the `moonup-shim` binary named after the toolchain command.
_Avoid_: Launcher, proxy

**Retire**:
Moving a replaced executable into the hidden `MOON_HOME/bin/.trash/` directory
under a unique name so the live name is freed immediately, deleting it best-effort
once any process holding it has exited. On Windows a running executable can be
renamed but not deleted, which is why retirement precedes deletion.
_Avoid_: Delete in place

### Release ordering

**Release order**:
The order in which installed toolchains are listed: pinned installs first,
grouped by channel and ordered oldest-to-newest by release date / index position;
floating channels follow their order in the cached main index, with channels
absent from the cached index sorted last, by name.
_Avoid_: Alphabetical order, install name order

**Channel release sequence**:
An oldest-to-newest sequence of releases provided by a channel index. Its
sequence position is authoritative, including when multiple releases share the
same base version but differ in build metadata.
_Avoid_: Sorted version list, numeric version order

**Concrete release identity**:
The exact version string of a release as it appears in a channel release
sequence, including build metadata such as `0.10.6+old` when present. It
identifies one channel member and is the only operand accepted by reusable
release comparison.
_Avoid_: Base version, partial version, version selector

**Release comparison**:
The channel-defined ordering between two concrete release identities. It returns
older, equal, or newer according to sequence position; callers apply relational
operators such as `>` or `>=` themselves.
_Avoid_: Numeric version comparison, threshold resolution
