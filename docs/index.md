---
layout: home

hero:
  name: "moonup"
  text: "Manage MoonBit toolchains with ease"
  tagline: The easy way to install, switch, and manage multiple MoonBit toolchains
  actions:
    - theme: brand
      text: Get Started
      link: /guide/
    - theme: alt
      text: View on GitHub
      link: https://github.com/chawyehsu/moonup

features:
  - icon: ⚡
    title: Quick Install
    details: Install any MoonBit toolchain version with a single command. Supports latest, nightly, and specific versions.
  - icon: 📌
    title: Project Pinning
    details: Pin a specific toolchain version to your project. moonup automatically switches when you enter the directory.
  - icon: 🔄
    title: Automatic Switching
    details: Shim executables transparently route commands to the correct toolchain. No manual PATH management needed.
  - icon: 🚀
    title: CI Ready
    details: First-class GitHub Actions support with the setup-moonup action. Perfect for reproducible builds.
---

## Quick Start

Install moonup with your preferred method:

::: code-group

```sh [cargo-binstall]
cargo-binstall moonup
```

```sh [cargo]
cargo install moonup
```

```sh [homebrew]
brew install chawyehsu/brew/moonup
```

```sh [pixi]
pixi global install moonup -c chawyehsu -c conda-forge
```

:::

Then install a MoonBit toolchain:

```sh
moonup install latest
```

That's it! You're ready to start building with MoonBit.
