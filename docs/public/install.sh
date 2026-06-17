#!/bin/sh
# moonup universal installer script
# Usage: curl -fsSL https://moonup.csu.moe/install.sh | sh

set -eu

# Check if moonup is already installed
if command -v moonup >/dev/null 2>&1; then
    echo "moonup is already installed, run moonup self-update to update"
    exit 0
fi

# Try package managers
try_brew() {
    if ! command -v brew >/dev/null 2>&1; then return 1; fi
    echo "Installing via Homebrew..."
    brew install chawyehsu/brew/moonup
}

try_cargo_binstall() {
    if ! command -v cargo-binstall >/dev/null 2>&1; then return 1; fi
    echo "Installing via cargo-binstall..."
    cargo-binstall moonup
}

try_pixi() {
    if ! command -v pixi >/dev/null 2>&1; then return 1; fi
    echo "Installing via pixi..."
    pixi global install moonup -c chawyehsu -c conda-forge
}

try_cargo() {
    if ! command -v cargo >/dev/null 2>&1; then return 1; fi
    echo "Installing via cargo..."
    cargo install moonup --locked
}

# Download from GitHub releases
install_from_github() {
    # Detect platform and architecture
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin)
            case "$arch" in
                arm64) target="aarch64-apple-darwin" ;;
                *)     echo "unsupported macOS architecture: $arch"; exit 1 ;;
            esac
            ;;
        Linux)
            case "$arch" in
                x86_64)  target="x86_64-unknown-linux-gnu" ;;
                aarch64) target="aarch64-unknown-linux-gnu" ;;
                arm64)   target="aarch64-unknown-linux-gnu" ;;
                *)       echo "unsupported Linux architecture: $arch"; exit 1 ;;
            esac
            ;;
        *)
            echo "unsupported operating system: $os"
            exit 1
            ;;
    esac
    case "$target" in
        *windows*) ext="zip" ;;
        *)       ext="tar.gz" ;;
    esac
    asset="moonup-${target}.${ext}"
    base_url="https://github.com/chawyehsu/moonup/releases/latest/download"
    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    echo "Downloading ${asset}..."
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${base_url}/${asset}" -o "${tmpdir}/${asset}"
        curl -fsSL "${base_url}/${asset}.sha256" -o "${tmpdir}/${asset}.sha256"
    elif command -v wget >/dev/null 2>&1; then
        wget -q "${base_url}/${asset}" -O "${tmpdir}/${asset}"
        wget -q "${base_url}/${asset}.sha256" -O "${tmpdir}/${asset}.sha256"
    else
        echo "curl or wget is required"
        exit 1
    fi

    # Verify checksum
    echo "Verifying checksum..."
    expected=$(cat "${tmpdir}/${asset}.sha256" | tr -d '[:space:]')
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "${tmpdir}/${asset}" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "${tmpdir}/${asset}" | awk '{print $1}')
    else
        echo "skipping checksum verification (sha256sum/shasum not found)"
        actual="$expected"
    fi
    if [ "$actual" != "$expected" ]; then
        echo "checksum mismatch: expected $expected, got $actual"
        exit 1
    fi

    # Extract
    install_dir="${HOME}/.moonup/bin"
    mkdir -p "$install_dir"
    echo "Installing to ${install_dir}..."

    case "$ext" in
        tar.gz) tar -xzf "${tmpdir}/${asset}" -C "${tmpdir}" ;;
        zip)    unzip -qo "${tmpdir}/${asset}" -d "${tmpdir}" ;;
    esac

    for bin in moonup moonup-shim; do
        if [ -f "${tmpdir}/${bin}" ]; then
            install -m 755 "${tmpdir}/${bin}" "${install_dir}/${bin}"
        fi
    done

    # Check if install_dir is in PATH
    case ":${PATH}:" in
        *":${install_dir}:"*) ;;
        *)
            echo "${install_dir} is not in your PATH"
            echo "Add it to your shell profile:"
            echo
            echo "  export PATH=\"\$HOME/.moonup/bin:\$PATH\""
            echo
            ;;
    esac
}

# Main
try_brew || try_cargo_binstall || try_pixi || try_cargo || install_from_github

# Verify installation
if command -v moonup >/dev/null 2>&1; then
    echo "moonup installed successfully!"
    echo "Run moonup install latest to install a MoonBit toolchain."
else
    echo "moonup was installed but is not in your PATH for this session"
    echo "Start a new shell or run:"
    echo
    echo "  export PATH=\"\$HOME/.moonup/bin:\$PATH\""
    echo
    echo "Then run moonup install latest to install a MoonBit toolchain."
fi
