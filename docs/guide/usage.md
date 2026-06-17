# Usage

This guide covers the most common moonup commands and workflows.

## Installing Toolchains

Install the latest stable toolchain:

```sh
moonup install latest
```

Install the latest nightly toolchain:

```sh
moonup install nightly
```

Install a specific version (the `v` prefix is optional):

```sh
moonup install 0.1.20241231+ba15a9a4e
```

## Listing Installed Toolchains

See all installed and active toolchains:

```sh
moonup list
```

## Setting the Default Toolchain

The default toolchain is used when no toolchain is specified in a project. By default, `latest` is used.

```sh
moonup default nightly
```

## Pinning a Toolchain to a Project

Pin a specific toolchain version to your project. This creates a `moonbit-version` file in the current directory:

```sh
moonup pin 0.1.20241231+ba15a9a4e
```

When you run MoonBit commands in this directory (or any subdirectory), moonup will automatically use the pinned version. You don't need to install the pinned version in advance — moonup will download and install it automatically.

To unpin, simply remove the `moonbit-version` file:

```sh
rm moonbit-version
```

## Running Commands with a Specific Toolchain

Run a command with a specific toolchain version:

```sh
moonup run 0.1.20241231+ba15a9a4e moon version
```

Or use the `+<spec>` syntax directly with moon commands:

```sh
moon +nightly version --all
```

## Uninstalling Toolchains

Uninstall a specific toolchain:

```sh
moonup uninstall 0.1.20241231+ba15a9a4e
```

Remove all cached downloads:

```sh
moonup uninstall --clear
```

## Using moonup in GitHub Actions

With the [setup-moonup](https://github.com/chawyehsu/setup-moonup) action, it's easy to set up a MoonBit environment in GitHub CI:

```yaml
- name: Setup MoonBit
  uses: chawyehsu/setup-moonup@v2
  run: moon version --all
```

## Distribution Server

moonup is backed by [chawyehsu/moonbit-binaries](https://github.com/chawyehsu/moonbit-binaries), a service built with GitHub Actions to continuously archive MoonBit releases from the official website and provide a distribution server with a stable API.

The default dist server endpoint is `https://moonup.csu.moe/v3`. You can override this with the `MOONUP_DIST_SERVER` environment variable:

```sh
export MOONUP_DIST_SERVER=https://moonup.corporate.internal/
```

This is useful when the default server is not accessible in your environment, or when you want to host your own distribution server.

## Shell Completions

Generate shell completions for your shell:

```sh
# For bash
moonup completions bash > ~/.bash_completion.d/moonup

# For zsh
moonup completions zsh > ~/.zfunc/_moonup

# For fish
moonup completions fish > ~/.config/fish/completions/moonup.fish
```
