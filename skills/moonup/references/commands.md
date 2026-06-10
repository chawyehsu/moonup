# Command Reference

## Run with Specific Toolchain

```sh
# Run any command with a specific toolchain
moonup run latest moon build
moonup run nightly moonc --version
moonup run 0.1.20241231+ba15a9a4e moon test

# Short alias
moonup x latest moon build
```

### Inline `+<spec>` Syntax

Shims also support inline toolchain selection without `moonup run`:

```sh
moon +nightly version --all
moon +0.1.20241231 build
moonc +latest --version
```

This is the highest-priority resolution method — it overrides pin files,
defaults, and env vars.

## List Installed Toolchains

```sh
moonup list
moonup ls
```

Shows all installed toolchains with the default marked and the currently active
one indicated.

## Show Which Binary Runs

```sh
moonup which moon
moonup which moonc
moonup which moon-lsp
```

Prints the actual path to the binary that would be executed for the given
command, after toolchain resolution.

## Shell Completions

```sh
moonup completions bash
moonup completions zsh
moonup completions fish
moonup completions elvish
moonup completions powershell
```

Typically redirected to the appropriate completion directory. Example for zsh:

```sh
moonup completions zsh > /usr/local/share/zsh/site-functions/_moonup
```

## Global Options

| Flag | Description |
| ------ | ------------- |
| `-v, --verbose` | Increase logging verbosity (repeatable) |
| `-q, --quiet` | Decrease logging verbosity (repeatable) |
| `-h, --help` | Print help |
| `-V, --version` | Print moonup version |
