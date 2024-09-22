# moonup

> Manage multiple [MoonBit] installations

[![cicd][cicd-badge]][cicd]
[![release][release-badge]][releases]
[![crates-svg]][crates-url]
[![license][license-badge]](LICENSE)
[![downloads][downloads-badge]][releases]

## Getting Started

If you have the Rust toolchain installed, you can install **moonup** easily with Cargo:

```sh
cargo install moonup
```

If you are on Windows and you have Scoop installed:

```pwsh
scoop bucket add dorado https://github.com/chawyehsu/dorado
scoop install moonup
```

Or you may download the latest release from [GitHub releases][releases],
manually extract the archive and put the executables in a directory that is in your `PATH`.

After installation, run `moonup help` to see the available commands.

```sh
$ moonup help
Moonup is a tool to manage multiple MoonBit installations.

If you find any bugs or have a feature request, please open an issue on
GitHub: https://github.com/chawyehsu/moonup/issues

Usage: moonup [OPTIONS] <COMMAND>

Commands:
  completions  Generate shell completions
  default      Set the default toolchain
  install      Install or update a MoonBit toolchain [aliases: i]
  pin          Pin the MoonBit toolchain to a specific version
  run          Run a command with a specific toolchain
  show         Show installed and currently active toolchains
  update       Update MoonBit latest toolchain and moonup [aliases: u]
  which        Show the actual binary that will be run for a given command
  help         Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  Increase logging verbosity
  -q, --quiet...    Decrease logging verbosity
  -h, --help        Print help
  -V, --version     Print version
```

### Use Moonup in GitHub Actions

With the [setup-moonup] action, It's super easy to use Moonup to set up a
MoonBit environment in GitHub CI. Here is an example workflow:

```yaml
- name: Setup MoonBit
  uses: chawyehsu/setup-moonup@v1
  run: moon version --all
```

## How It Works

Moonup allows you to install multiple MoonBit toolchains and switch between
them easily. All MoonBit toolchains (with the core standard library) are
installed in Moonup's `toolchains` directory.

Moonup creates shim executables that replace the original MoonBit
executables in the PATH. When you run a MoonBit command, the shim
executable determines which MoonBit toolchain to use and proxies the
command to the actual MoonBit executable in the desired toolchain.

With this approach, you can switch between MoonBit toolchains across
projects easily without needing to change the PATH.

### MoonBit Releases

Moonup downloads MoonBit releases from [chawyehsu/moonbit-binaries],
which is powered by GitHub Actions and archives MoonBit releases
continuously from the official website.

### Known Limitations

- Isolation of MoonBit core standard library is problematic, see #7.

## Development

Prerequisites: Git, Rust

```sh
# clone the repo
git clone https://github.com/chawyehsu/moonup
cd moonup
# build
cargo build
# run and test
cargo run -- help
```

## 0.1.0 Roadmap

- [x] An `install` command to install multiple MoonBit toolchains
- [x] A `pin` command to pin toolchain to a specific version in a project
- [x] Create shim executables to switch between toolchains automatically
- [x] A `default` command to set the default toolchain
- [x] A `show` command to show installed and currently active toolchains
- [x] A `which` command to show the actual binary that will be run for a given command
- [x] A `run` command to run a command with a specific toolchain
- [x] A `update` command to self-update and update the toolchain
- [x] A `completions` command to generate shell completions

## License

**moonup** © [Chawye Hsu](https://github.com/chawyehsu). Released under the [Apache-2.0](LICENSE) license.

> [Blog](https://chawyehsu.com) · GitHub [@chawyehsu](https://github.com/chawyehsu) · Twitter [@chawyehsu](https://twitter.com/chawyehsu)

[MoonBit]: https://www.moonbitlang.com/
[cicd-badge]: https://github.com/chawyehsu/moonup/workflows/CICD/badge.svg
[cicd]: https://github.com/chawyehsu/moonup/actions/workflows/cicd.yml
[release-badge]: https://img.shields.io/github/v/release/chawyehsu/moonup
[releases]: https://github.com/chawyehsu/moonup/releases/latest
[crates-svg]: https://img.shields.io/crates/v/moonup.svg
[crates-url]: https://crates.io/crates/moonup
[license-badge]: https://img.shields.io/github/license/chawyehsu/moonup
[downloads-badge]: https://img.shields.io/github/downloads/chawyehsu/moonup/total
[setup-moonup]: https://github.com/chawyehsu/setup-moonup
[chawyehsu/moonbit-binaries]: https://github.com/chawyehsu/moonbit-binaries
