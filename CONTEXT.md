# Moonup

A version manager that installs and manages multiple MoonBit toolchains. Operations that replace an installed toolchain must never leave it corrupted.

## Language

**Toolchain**:
A named, self-contained installation of the MoonBit compiler and its components, stored under `MOON_HOME/toolchains/<name>`.
_Avoid_: Install, build, distribution

**Toolchain spec**:
The identifier used to select a toolchain: `latest`, `nightly`, `bleeding`, or a specific version (e.g. `v1.0.0` or `nightly-2025-01-01`).
_Avoid_: Channel, tag, release

**Atomic toolchain change**:
An operation that replaces an installed toolchain such that the currently installed toolchain remains fully usable at every instant, even if the operation fails or the process is killed mid-way.
_Avoid_: Safe delete, clean replace

**Staging directory**:
Where a new toolchain is fully assembled and verified before it is swapped into the live install location. A failed install leaves a discarded staging directory, never a corrupted live toolchain.

**Populate**:
The act of assembling a toolchain into its install location: downloading components, extracting them, and making the toolchain live.
_Avoid_: Install in place
