# Install moonup

moonup must be installed before it can manage MoonBit toolchains. Choose the
method that matches your environment.

## Recommended Order

### 1. pixi (cross-platform, recommended)

If you have [pixi](https://pixi.sh) installed, this is the recommended method
across all platforms:

```sh
pixi global install moonup -c chawyehsu -c conda-forge
```

### 2. Homebrew (macOS)

```zsh
brew install chawyehsu/brew/moonup
```

### 3. cargo-binstall (cross-platform, pre-built binaries)

If you have [cargo-binstall](https://github.com/cargo-bins/cargo-binstall),
this downloads pre-built binaries without compiling:

```sh
cargo-binstall moonup
```

### 4. cargo (cross-platform, builds from source)

Requires a Rust toolchain. Slower since it compiles from source:

```sh
cargo install moonup
```

### 5. Scoop (Windows)

```pwsh
scoop bucket add dorado https://github.com/chawyehsu/dorado
scoop install moonup
```

### 6. GitHub Releases (manual)

Download from [GitHub releases](https://github.com/chawyehsu/moonup/releases/latest),
extract the archive, and place the executables in a `PATH` directory.

## Post-Install Setup

After installing moonup, install the MoonBit toolchain:

```sh
moonup install latest
```

Ensure `~/.moon/bin` is in your `PATH` so that `moon`, `moonc`, `moonrun`,
and other commands are available.
