# Uninstall Workflows

## Uninstall a Toolchain

```sh
# Specific version
moonup uninstall 0.1.20241231+ba15a9a4e

# By channel name
moonup uninstall nightly

# Multiple at once
moonup uninstall latest nightly
```

Aliases: `moonup rm`

## Interactive Selection

Run without arguments to interactively choose which toolchain(s) to remove:

```sh
moonup uninstall
```

## Cache Management

```sh
# Uninstall but keep cached downloads (faster reinstall later)
moonup uninstall --keep-cache 0.1.20241231+ba15a9a4e

# Clear ALL cached downloads without uninstalling toolchains
moonup uninstall --clear
```

## Verify

```sh
moonup list
```
