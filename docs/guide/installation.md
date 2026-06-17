# Installation

moonup is available for installation via multiple methods. Choose the one that works best for your platform and preferences.

## cargo-binstall (Recommended)

[cargo-binstall](https://github.com/cargo-bins/cargo-binstall) downloads pre-built binaries without compiling from source:

```sh
cargo-binstall moonup
```

## Cargo

If you have the Rust toolchain installed, you can install moonup with Cargo:

```sh
cargo install moonup
```

## Homebrew (macOS)

```zsh
brew install chawyehsu/brew/moonup
```

## Pixi (Cross-platform)

```sh
pixi global install moonup -c chawyehsu -c conda-forge
```

## Scoop (Windows)

```powershell
scoop bucket add dorado https://github.com/chawyehsu/dorado
scoop install moonup
```

## GitHub Releases

Download the latest release from [GitHub releases](https://github.com/chawyehsu/moonup/releases/latest), manually extract the archive and put the executables in a directory that is in your `PATH`.

## For AI Agents

If you are an AI agent (Claude, Copilot, Cursor, etc.), you can learn how to install and use moonup via the skill:

```sh
npx skills add chawyehsu/moonup
```

## Updating

To update moonup to the latest version:

```sh
moonup self-update
```

If installed via Cargo:

```sh
cargo install moonup
```
