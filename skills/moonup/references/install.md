# Install Workflows

## Install a Toolchain

```sh
# Latest stable
moonup install latest

# Nightly build
moonup install nightly

# Specific version (no "v" prefix)
moonup install 0.1.20241231+ba15a9a4e

# Specific nightly by date
moonup install nightly-2026-01-01
```

Aliases: `moonup i`

## List Available Versions

```sh
# All channels
moonup install --list-available

# Specific channel
moonup install latest --list-available
moonup install nightly --list-available
```

## First-Time Setup

```sh
moonup install latest
# Optional since `latest` is the default, but you can set it explicitly
moonup default latest
# Ensure ~/.moon/bin is in PATH
```

## Auto-Install via Shims

When a user runs `moon`, `moonc`, `moonrun`, or other MoonBit commands through
the shims in `~/.moon/bin/`, the shim checks if the required toolchain is
installed. If not, it automatically runs `moonup install` for the resolved
toolchain. No manual install step is needed in most cases, just running
`moon` subcommands in a project with a `moonbit-version` file will trigger installation.

## Deprecated Channels

The `bleeding` channel is deprecated and no longer available. Use `nightly` as
the alternative for latest builds.

## Verify Installation

```sh
moonup list
moon version --all
```
