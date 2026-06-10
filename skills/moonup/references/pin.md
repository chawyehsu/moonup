# Pin & Default Toolchain

## Set Global Default

```sh
# Set explicitly
moonup default latest
moonup default nightly

# Interactive selection (no argument)
moonup default
```

The default is used when no other toolchain resolution rule matches.

## Pin to a Project

```sh
# Pin current directory to a specific version
moonup pin 0.1.20241231+ba15a9a4e

# Pin to a channel
moonup pin nightly

# Interactive selection
moonup pin
```

This creates a `moonbit-version` file in the current directory. Commit this
file to version control so all collaborators use the same toolchain.

To unpin, delete the file:

```sh
rm moonbit-version
```

## Toolchain Resolution Order

When running any MoonBit command (moon, moonc, etc.), the active toolchain is
determined by (highest priority first):

1. **Inline spec**: `+<spec>` argument — e.g., `moon +nightly build`
2. **Env var**: `MOONUP_TOOLCHAIN_SPEC` (set by parent process)
3. **Pin file**: `moonbit-version` in cwd or nearest parent
4. **Default**: `~/.moonup/default`
5. **Fallback**: `latest`

This means a project-level pin always overrides the global default, and an
inline `+<spec>` overrides everything.
