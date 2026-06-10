---
name: moonup-github-actions
description: >
  Set up MoonBit toolchain in GitHub Actions CI using the setup-moonup action.
  Use this skill whenever the user needs to configure MoonBit in GitHub Actions,
  set up a CI pipeline for a MoonBit project, or mentions "setup-moonup",
  "moonbit ci", "moonbit github actions", or "moonbit pipeline".
---

# setup-moonup — GitHub Action for MoonBit CI

The [setup-moonup](https://github.com/chawyehsu/setup-moonup) action installs
Moonup and a MoonBit toolchain in GitHub Actions runners.

## Basic Usage

```yaml
- uses: chawyehsu/setup-moonup@v2
- run: moon version --all
```

This installs the latest moonup, then the latest stable MoonBit toolchain.

## Inputs

| Input | Required | Default | Description |
|-------|----------|---------|-------------|
| `version` | No | (latest) | Pin a moonup version, e.g. `0.5.2` or `v0.5.2` |
| `moonbit-version` | No | `latest` | MoonBit version, e.g. `latest`, `nightly`, `0.1.20241231+ba15a9a4e` |
| `token` | No | `github.token` | GitHub token for API requests |

## Supported Runners

| Runner | Architecture |
|--------|-------------|
| `ubuntu-latest` | x86_64, aarch64 |
| `macos-latest` | aarch64 (Apple Silicon) |
| `windows-latest` | x86_64 |

## Common Patterns

See [references/examples.md](references/examples.md) for full workflow examples
including nightly builds, matrix testing, and pinned versions.
