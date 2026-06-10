# Update Workflows

## Update Toolchains

```sh
moonup update
```

Aliases: `moonup u`

This updates the `latest` and `nightly` channels to their newest available
versions.

## Update moonup Itself

```sh
moonup self-update
```

> **Caveat**: This command is only available when moonup was built with the
> `self_update` cargo feature flag. If the command is not found, then moonup
> was likely installed without self-update support (e.g., installed by 3rd-party
> package managers or built from source without the feature). Try to update with
> your package manager or by reinstalling it from the release source such as
> `cargo install moonup`. Ask the user how they installed moonup if they are unsure.

## Verify After Update

```sh
moonup list
moon version --all
```
