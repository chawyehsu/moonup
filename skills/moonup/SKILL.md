---
name: moonup
description: >
  Manage MoonBit toolchain versions using moonup — install moonup itself, install,
  update, uninstall, pin, switch, and run commands with specific toolchains.
  Use this skill whenever the user needs to install moonup, install or set up
  MoonBit, switch between toolchain versions (latest, nightly), pin a version
  to a project, troubleshoot a missing or wrong MoonBit toolchain version, or
  manage MoonBit toolchain lifecycle. Also trigger on "install moonup",
  "moon not found", "set up MoonBit", "update moonbit", "switch moonbit version",
  or any moonup CLI usage.
---

# Moonup — MoonBit Toolchain Version Manager

Moonup manages multiple MoonBit toolchain installations side by side. It installs
toolchains to `~/.moonup/toolchains/` and creates shims in `~/.moon/bin/` that
automatically select the right toolchain at runtime.

## Key Behaviors

- **Auto-install**: Running `moon` (or `moonc`, `moonrun`, etc.) via shims
  auto-installs the required toolchain if not already present.
- **Toolchain resolution order** (highest priority first):
  1. `+<spec>` inline syntax (e.g., `moon +nightly build`)
  2. `MOONUP_TOOLCHAIN_SPEC` env var
  3. `moonbit-version` file in current/parent directory
  4. Default toolchain (`~/.moonup/default`)
  5. Falls back to `latest`
- **Channels**: `latest` (stable), `nightly`, or a specific version tag like
  `0.1.20241231+ba15a9a4e`.
- **Deprecated**: The `bleeding` channel is deprecated. Use `nightly` instead.

## Quick Start

If moonup is not installed yet, see [references/setup.md](references/setup.md).

```sh
# Install latest stable MoonBit toolchain
moonup install latest

# Set it as default (This is optional since `latest` is the default)
moonup default latest

# Verify
moon version --all
```

## Workflow Routing

| Task | Reference |
|------|-----------|
| Install moonup itself | [references/setup.md](references/setup.md) |
| Install a toolchain | [references/install.md](references/install.md) |
| Update toolchains | [references/update.md](references/update.md) |
| Uninstall / clean up | [references/uninstall.md](references/uninstall.md) |
| Pin version / set default | [references/pin.md](references/pin.md) |
| Run with specific toolchain, list, which, completions | [references/commands.md](references/commands.md) |

Read the relevant reference file before executing the corresponding workflow.

## Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `MOONUP_HOME` | `~/.moonup` | moonup data directory |
| `MOON_HOME` | `~/.moon` | MoonBit home (shims live in `bin/`) |
| `MOONUP_DIST_SERVER` | `https://moonup.csu.moe/v3` | Distribution server |

## Shell Setup

Shell completions are available for bash, zsh, fish, elvish, and powershell.
Generate shell completions with:

```sh
moonup completions <bash|zsh|fish|elvish|powershell>
```
