# Moonup

A version manager that installs and manages multiple MoonBit toolchains. Operations that replace an installed toolchain must never leave it corrupted.

## Language

**Toolchain**:
A named, self-contained installation of the MoonBit compiler and its components, stored under `MOON_HOME/toolchains/<name>`.
_Avoid_: Install, build, distribution

**Toolchain spec**:
The identifier used to select a toolchain: `latest`, `nightly`, `bleeding`, or a specific version (e.g. `1.0.0` or `nightly-2025-01-01`).
_Avoid_: Channel, tag, release

**Version selector**:
A numeric stable-version input accepted by the install command. It may contain one, two, or three dot-separated components (for example, `0`, `0.10`, or `0.10.1`). A selector chooses a concrete stable release; it is not itself an installed toolchain identity.

**Resolved release**:
The concrete stable release selected from the ordered stable release index for a version selector. The resolved release, rather than the selector, is the toolchain's installation identity.

**Atomic toolchain change**:
An operation that replaces an installed toolchain such that the currently installed toolchain remains fully usable at every instant, even if the operation fails or the process is killed mid-way.
_Avoid_: Safe delete, clean replace

**Staging directory**:
Where a new toolchain is fully assembled and verified before it is swapped into the live install location. A failed install leaves a discarded staging directory, never a corrupted live toolchain.

**Populate**:
The act of assembling a toolchain into its install location: downloading components, extracting them, and making the toolchain live.
_Avoid_: Install in place

**Shim**:
A wrapper executable in `MOON_HOME/bin/` (and `bin/internal/`) that forwards a toolchain command (e.g. `moon`) to the active toolchain's real binary via moonup. It is a copy of the `moonup-shim` binary named after the toolchain command.
_Avoid_: Launcher, proxy

**Release order**:
The order in which installed toolchains are listed: versioned installs first, grouped by channel and ordered oldest-to-newest by release date, then the floating channel aliases in `bleeding` → `nightly` → `latest` order.
_Avoid_: Alphabetical order, install name order

**Retire**:
Moving a replaced executable into the hidden `MOON_HOME/bin/.trash/` directory under a unique name so the live name is freed immediately, deleting it best-effort once any process holding it has exited. On Windows a running executable can be renamed but not deleted, which is why retirement precedes deletion.
_Avoid_: Delete in place
