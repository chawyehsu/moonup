# Install moonup

moonup must be installed before it can manage MoonBit toolchains. Choose the
method that matches your environment.

## Recommended Order

### 1. Universal Installer (Recommended)

This is the recommended method for installing moonup across all platforms.

For Unix-like systems, use the following command:

```sh [Unix]
curl -fsSL https://moonup.csu.moe/install | sh
```

And for Windows systems, use the following command:

```sh [Windows]
irm https://moonup.csu.moe/install | iex
```

It'll detect and use the appropriate installation method for your platform.
Optionally, you can also install moonup via other methods described below.

### 2. pixi (cross-platform)

If you have [pixi](https://pixi.sh) installed, this is the recommended method
across all platforms:

```sh
pixi global install moonup -c chawyehsu -c conda-forge
```

### 3. Homebrew (macOS)

```zsh
brew install chawyehsu/brew/moonup
```

### 4. cargo-binstall (cross-platform, pre-built binaries)

If you have [cargo-binstall](https://github.com/cargo-bins/cargo-binstall),
this downloads pre-built binaries without compiling:

```sh
cargo-binstall moonup
```

### 5. cargo (cross-platform, builds from source)

Requires a Rust toolchain. Slower since it compiles from source:

```sh
cargo install moonup
```

### 6. Scoop (Windows)

```pwsh
scoop bucket add dorado https://github.com/chawyehsu/dorado
scoop install moonup
```

### 7. GitHub Releases (manual)

Download from [GitHub releases](https://github.com/chawyehsu/moonup/releases/latest),
extract the archive, and place the executables in a `PATH` directory.

## Post-Install Setup

After installing moonup, install the MoonBit toolchain:

```sh
moonup install latest
```

Ensure `~/.moon/bin` is in your `PATH` so that `moon`, `moonc`, `moonrun`,
and other commands are available.
