# Fixture stub

A tiny native executable that replaces every binary in a shrunken toolchain
fixture archive.

The source is a C program. It is designed to be small and cross-compiled to
each CI target with `zig cc`.

## Build

The binaries are build output, not committed. Generate them with:

```sh
tests/fixtures/stub/gen.sh
```

This produces `out/<triplet>-stub` for each CI target. The `-stub` suffix
keeps the name generic so the same binary can be reused as, e.g., a `moonc`
stub when the fixture archives later grow other executables.
`scripts/update_fixture.sh` runs `gen.sh` automatically before assembling the
shrunken archives.

Linux stubs are static (musl) and run on the glibc CI runners; the Windows
stub is a UCRT PE, which ships with Windows 10+ and on `windows-latest` CI.
