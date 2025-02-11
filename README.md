# moonup

> Manage multiple [MoonBit] installations

[![cicd][cicd-badge]][cicd]
[![downloads][downloads-badge]][releases]
[![release][release-badge]][releases]
[![crates-svg]][crates-url]
[![license][license-badge]](LICENSE)
[![codecov][codecov-badge]][codecov]

## Getting Started

### Install

Moonup is available for installation via different ways.

#### Conda

You can install moonup with conda/mamba/[pixi] from our conda-forge channel:

```sh
pixi global install moonup -c chawyehsu -c conda-forge
```

#### Cargo

If you have the Rust toolchain installed, you can install **moonup** easily with Cargo:

```sh
cargo install moonup
```

#### Scoop (Windows)

If you are on Windows and you have Scoop installed:

```pwsh
scoop bucket add dorado https://github.com/chawyehsu/dorado
scoop install moonup
```

#### GitHub Releases

Or you may download the latest release from [GitHub releases][releases],
manually extract the archive and put the executables in a directory that is in your `PATH`.

### Usage

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
  list         List installed and active toolchains [aliases: ls]
  pin          Pin the MoonBit toolchain to a specific version
  run          Run a command with a specific toolchain
  self-update  Update Moonup to the latest version
  uninstall    Uninstall a MoonBit toolchain
  update       Update MoonBit toolchains [aliases: u]
  which        Show the actual binary that will be run for a given command
  help         Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose...  Increase logging verbosity
  -q, --quiet...    Decrease logging verbosity
  -h, --help        Print help
  -V, --version     Print version
```

#### Install a MoonBit Toolchain

```sh
# install the latest MoonBit toolchain
moonup install latest
# install the latest nightly MoonBit toolchain
moonup install nightly
# install a specific MoonBit toolchain (`v` prefix is not required)
moonup install 0.1.20241231+ba15a9a4e
```

#### Pin a MoonBit Toolchain

By pinning a MoonBit toolchain to a specific version in a project, the toolchain
will be automatically switched to the pinned version when you're running MoonBit
commands in the project directory. With this feature, you can ensure that the
project is built and tested with the same MoonBit toolchain.

```sh
moonup pin 0.1.20241231+ba15a9a4e
# remove the `moonbit-version` file to unpin
rm moonbit-version
```

You're not required to install the pinned MoonBit toolchain in advance. Moonup
will download and install the toolchain automatically when a command is run.

#### Set the Default Toolchain

The default toolchain is used when no toolchain is specified in a project. The
`latest` toolchain is used by default.

```sh
# change the default toolchain to the latest nightly
moonup default nightly
```

#### Check Installed and Active Toolchains

```sh
moonup list
```

#### Run a Command with a Specific Toolchain

```sh
moonup run 0.1.20241231+ba15a9a4e moon version
# or pass the version to the command directly! (syntax: `+<spec>`)
moon +nightly version --all
```

#### Uninstall a MoonBit Toolchain

```sh
# relevant cached downloads will be removed as well
moonup uninstall 0.1.20241231+ba15a9a4e
# delete all cached downloads
moonup uninstall --clear
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

- Isolation of MoonBit core standard library is problematic, see [#7].

## Development

Prerequisites: Git, [pixi]

```sh
# clone the repo
git clone https://github.com/chawyehsu/moonup
cd moonup
# install dependencies
pixi install
# install git pre-commit hooks
pixi run pre-commit install
# dev build and run
pixi run cargo run moonup -- help
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
[cicd-badge]: https://img.shields.io/github/actions/workflow/status/chawyehsu/moonup/cicd.yml?style=flat&logo=github&logoColor=FFFFFF&colorA=0B031E&colorB=9E1084
[cicd]: https://github.com/chawyehsu/moonup/actions/workflows/cicd.yml
[release-badge]: https://img.shields.io/github/v/release/chawyehsu/moonup?style=flat&logo=semanticrelease&logoColor=FFFFFF&colorA=0B031E&colorB=9E1084
[releases]: https://github.com/chawyehsu/moonup/releases/latest
[crates-svg]: https://img.shields.io/crates/v/moonup.svg?style=flat&logo=rust&logoColor=FFFFFF&colorA=0B031E&colorB=9E1084
[codecov-badge]: https://img.shields.io/codecov/c/gh/chawyehsu/moonup?style=flat&logo=codecov&logoColor=FFFFFF&colorA=0B031E&colorB=9E1084
[codecov]: https://codecov.io/github/chawyehsu/moonup
[crates-url]: https://crates.io/crates/moonup
[license-badge]: https://img.shields.io/github/license/chawyehsu/moonup?style=flat&logo=spdx&logoColor=FFFFFF&colorA=0B031E&colorB=9E1084
[downloads-badge]: https://img.shields.io/github/downloads/chawyehsu/moonup/total?style=flat&logo=github&logoColor=FFFFFF&colorA=0B031E&colorB=9E1084
[pixi]: https://pixi.sh
[setup-moonup]: https://github.com/chawyehsu/setup-moonup
[chawyehsu/moonbit-binaries]: https://github.com/chawyehsu/moonbit-binaries
[#7]: https://github.com/chawyehsu/moonup/issues/7
