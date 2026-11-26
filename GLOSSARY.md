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

**Installed toolchain**:
An existing local toolchain installation, identified by its floating channel or
pinned version. A floating installation may also have a known current release
key.

**Default channel**:
The fixed-name channel `latest` that every dist server must advertise. Bare
version pins always resolve within it; the shim's network-free toolchain
fallback is also its name. It is a constant, not derived from index position.
_Avoid_: default toolchain (that term means something else), primary channel

**Channel**:
A named distribution line advertised by the dist server's main index (e.g. `latest`,
`nightly`). A channel has no fixed identity in Moonup: whatever names the index
lists are the channels that exist. Moonup attaches no semantics to any particular
name.
_Avoid_: branch, track

**Qualified pin**:
A pinned version that names its channel explicitly, written
`{channel}<sep>{key}` with `<sep>` `/` or `@` (e.g. `lts/0.11.0` or
`lts@0.11.0`), and canonically written with `@`. The legacy
`nightly-YYYY-MM-DD` spelling remains accepted for dated nightly pins. It exists
so the same release version can be pinned from different channels without the
installs colliding.
_Avoid_: channel-qualified version, namespaced pin

**Pinned version**:
A concrete release identified by its channel and release key: bare (e.g.
`0.11.0`, meaning the `latest` channel) or qualified (e.g. `lts/0.11.0`). Dated
builds use the build date as the key (e.g. `nightly@2026-11-19`; the legacy
`nightly-2026-11-19` spelling is also accepted).
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
unset, the literal name `latest` is the conventional fallback (also the default
channel's name).
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
the live install location. Verification is by the install record it holds. A
failed install leaves a discarded staging directory, never a corrupted live
toolchain.

**Populate**:
The act of assembling a toolchain: downloading components, extracting them
into a staging directory, and swapping the fully assembled result into the
live install location.
_Avoid_: Install in place

**Install record**:
The identity file inside a toolchain install directory, live or staged: the
channel, the release key, whether the install is floating, and — while
finalization is pending — the finalization inputs. A staging directory is
complete when it holds a matching install record with a finalization payload.
_Avoid_: version stub, metadata file, completeness marker

**Promote**:
The rename that moves a complete staging directory into the live install
location, preceded by retiring the previous live directory under a unique
name. If promotion fails, the retired toolchain is restored to the live name.
_Avoid_: swap in place, copy over

**Finalization**:
The post-install steps that make a promoted toolchain fully usable: pouring
shims, building the core library, and linking directories. Its inputs travel
in the install record so finalization never depends on the network.
_Avoid_: post-install, finalize step

**Pending finalization**:
The state of a promoted toolchain whose finalization has not been
acknowledged, recorded by the presence of the finalization inputs in its
install record. Recovery retries the post-install steps and acknowledges the
promotion by rewriting the record without them.
_Avoid_: incomplete install, unacknowledged install

**Shim**:
A wrapper executable in `MOON_HOME/bin/` (and `bin/internal/`) that forwards a
toolchain command (e.g. `moon`) to the active toolchain's real binary via moonup.
It is a copy of the `moonup-shim` binary named after the toolchain command.
_Avoid_: Launcher, proxy

**Retire**:
Moving a replaced artifact — a shim executable, or a replaced toolchain
directory — aside under a unique hidden name so the live name is freed
immediately, deleting it best-effort once any process holding it has exited.
On Windows a running executable can be renamed but not deleted, which is why
retirement precedes deletion.
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

### Invocation

**Session**:
One moonup invocation's span of authority: the locations moonup operates on
are resolved once at its start, and all reads of distribution-server metadata
within it see one consistent view. Every invocation, including each shim run,
is exactly one session, and sessions never observe each other's state.
_Avoid_: Context, config, cache, global state
