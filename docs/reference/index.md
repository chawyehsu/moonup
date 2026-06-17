# CLI Reference

This page documents all moonup commands and their options.

## Global Options

```
-v, --verbose...  Increase logging verbosity
-q, --quiet...    Decrease logging verbosity
-h, --help        Print help
-V, --version     Print version
```

## Commands

### `moonup install`

Install or update a MoonBit toolchain.

**Aliases:** `i`

**Usage:**

```sh
moonup install <SPEC>
```

**Arguments:**

- `<SPEC>` - Toolchain specification:
  - `latest` - Latest stable release
  - `nightly` - Latest nightly build
  - `0.1.20241231+ba15a9a4e` - Specific version (v prefix optional)

**Examples:**

```sh
moonup install latest
moonup install nightly
moonup install 0.1.20241231+ba15a9a4e
```

---

### `moonup list`

List installed and active toolchains.

**Aliases:** `ls`

**Usage:**

```sh
moonup list
```

**Example Output:**

```
Moonup home: /home/user/.moonup

Installed toolchains:
  0.6.22
  0.6.24+012953835
  0.6.29+9037370fc
  0.9.2+bbe2b338f
  latest (0.10.0+e66899a54)
  nightly (2026-06-11)

Active toolchain: latest
```

---

### `moonup default`

Set the default toolchain.

**Usage:**

```sh
moonup default <SPEC>
```

**Arguments:**

- `<SPEC>` - Toolchain specification

**Examples:**

```sh
moonup default nightly
moonup default 0.1.20241231+ba15a9a4e
```

---

### `moonup pin`

Pin the MoonBit toolchain to a specific version in the current project.

**Usage:**

```sh
moonup pin <SPEC>
```

**Arguments:**

- `<SPEC>` - Toolchain specification

**Examples:**

```sh
moonup pin 0.1.20241231+ba15a9a4e
```

This creates a `moonbit-version` file in the current directory. When MoonBit commands are run in this directory or its subdirectories, moonup will automatically use the pinned version.

---

### `moonup run`

Run a command with a specific toolchain.

**Usage:**

```sh
moonup run <SPEC> <COMMAND> [ARGS]...
```

**Arguments:**

- `<SPEC>` - Toolchain specification
- `<COMMAND>` - Command to run
- `[ARGS]...` - Arguments to pass to the command

**Examples:**

```sh
moonup run 0.1.20241231+ba15a9a4e moon version
moonup run nightly moon build
```

---

### `moonup which`

Show the actual binary that will be run for a given command.

**Usage:**

```sh
moonup which <COMMAND>
```

**Arguments:**

- `<COMMAND>` - Command to locate

**Examples:**

```sh
moonup which moon
# Output: /home/user/.moonup/toolchains/0.1.20241231+ba15a9a4e/bin/moon
```

---

### `moonup uninstall`

Uninstall a MoonBit toolchain.

**Usage:**

```sh
moonup uninstall [OPTIONS] [SPEC]
```

**Arguments:**

- `[SPEC]` - Toolchain specification (optional, if not specified, removes all)

**Options:**

- `--clear` - Delete all cached downloads

**Examples:**

```sh
moonup uninstall 0.1.20241231+ba15a9a4e
moonup uninstall --clear
```

---

### `moonup update`

Update MoonBit toolchains.

**Aliases:** `u`

**Usage:**

```sh
moonup update [SPEC]
```

**Arguments:**

- `[SPEC]` - Toolchain specification (optional, if not specified, updates all)

**Examples:**

```sh
moonup update
moonup update latest
```

---

### `moonup self-update`

Update Moonup to the latest version.

**Usage:**

```sh
moonup self-update
```

---

### `moonup completions`

Generate shell completions.

**Usage:**

```sh
moonup completions <SHELL>
```

**Arguments:**

- `<SHELL>` - Shell to generate completions for: `bash`, `zsh`, `fish`, `powershell`, `elvish`

**Examples:**

```sh
moonup completions bash > ~/.bash_completion.d/moonup
moonup completions zsh > ~/.zfunc/_moonup
moonup completions fish > ~/.config/fish/completions/moonup.fish
```

---

## Environment Variables

- `MOONUP_DIST_SERVER` - Override the default distribution server endpoint
- `MOONUP_HOME` - Override the moonup installation directory (default: `~/.moonup`)
