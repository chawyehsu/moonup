# GitHub Actions Workflow Examples

## Minimal Setup

```yaml
name: CI
on: [push, pull_request]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v6
      - uses: chawyehsu/setup-moonup@v2
      - run: moon version --all
      - run: moon build
      - run: moon test
```

## Pin MoonBit Version

Use a specific MoonBit version for reproducible builds:

```yaml
- uses: chawyehsu/setup-moonup@v2
  with:
    moonbit-version: '0.1.20241231+ba15a9a4e'
```

## Nightly Toolchain

```yaml
- uses: chawyehsu/setup-moonup@v2
  with:
    moonbit-version: nightly
```

## Pin Both Versions

Pin both moonup and MoonBit versions:

```yaml
- uses: chawyehsu/setup-moonup@v2
  with:
    version: '0.5.2'
    moonbit-version: '0.1.20241231+ba15a9a4e'
```

## Matrix Testing (Multiple Platforms)

```yaml
name: CI
on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v6
      - uses: chawyehsu/setup-moonup@v2
      - run: moon build
      - run: moon test
```

## Full Project Workflow

A more complete workflow with build, test, and formatting checks:

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v6
      - name: Install MoonBit
        uses: chawyehsu/setup-moonup@v2
      - run: moon check
      - run: moon test
      - run: moon fmt --check
```
