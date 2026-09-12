# Getting Started

moonup is a tool to manage multiple MoonBit installations, similar to how rustup manages Rust toolchains.

## Why moonup?

- **Multiple Versions**: Install and use different MoonBit toolchain versions side by side
- **Project Pinning**: Pin a specific toolchain version per project for reproducible builds
- **Transparent Switching**: Shim executables automatically route to the correct toolchain
- **Easy Updates**: Update all your toolchains with a single command

## Quick Installation

The fastest way to get started:

::: code-group

```sh [Unix]
curl -fsSL https://moonup.csu.moe/install | sh
```

```sh [Windows]
irm https://moonup.csu.moe/install | iex
```

:::

See the [Installation](./installation) guide for more options.

## Your First Toolchain

After installing moonup, install the latest MoonBit toolchain:

```sh
moonup install latest
```

Verify it's working:

```sh
moon version --all
```

You're ready to start building with MoonBit!

## Next Steps

- [Installation](./installation) - All installation methods
- [Usage](./usage) - Common commands and workflows
- [CLI Reference](/reference/) - Complete command reference
