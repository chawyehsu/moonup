# moonup

> Manage multiple [MoonBit] installations

[![cicd][cicd-badge]][cicd]
[![release][release-badge]][releases]
[![crates-svg]][crates-url]
[![license][license-badge]](LICENSE)
[![downloads][downloads-badge]][releases]

## Getting Started

If you have the Rust toolchain installed, you can simply install **moonup** with Cargo:

```sh
cargo install moonup@0.1.0-beta.2
```

Or you may download the latest release from [GitHub releases][releases],
manuallly extract the archive and put the executables in a directory that is in your `PATH`.

After instllation, run `moonup help` to see the available commands.

```sh
$ moonup help
Moonup is a tool to manage multiple MoonBit installations.

If you find any bugs or have a feature request, please open an issue on
GitHub: https://github.com/chawyehsu/moonup/issues

Usage: moonup.exe [OPTIONS] <COMMAND>

Commands:
  default  Set the default toolchain
  install  Install or update a MoonBit toolchain
  pin      Pin the MoonBit toolchain to a specific version
  show     Show installed and currently active toolchains
  which    Show the actual binary that will be run for a given command
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  Increase logging verbosity
  -q, --quiet...    Decrease logging verbosity
  -h, --help        Print help
  -V, --version     Print version
```

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
- [ ] A `run` command to run a command with a specific toolchain
- [ ] A `update` command to self-update and update the toolchain
- [ ] A `completions` command to generate shell completions

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
