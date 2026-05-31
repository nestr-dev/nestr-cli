#!/bin/sh
set -eu

REPO="nestr/nestr-cli"
BINARY_NAME="nestr"

main() {
    os="$(uname -s)"; arch="$(uname -m)"
    case "$os" in
        Darwin) os="apple-darwin" ;;
        Linux)  os="unknown-linux-musl" ;;
        *) echo "Unsupported OS: $os. Download from https://github.com/${REPO}/releases" >&2; exit 1 ;;
    esac
    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) echo "Unsupported arch: $arch" >&2; exit 1 ;;
    esac
    target="${arch}-${os}"

    if [ -n "${NESTR_VERSION:-}" ]; then
        version="$NESTR_VERSION"
    else
        version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\([^"]*\)".*/\1/p')"
    fi
    [ -n "$version" ] || { echo "Could not determine the latest ${BINARY_NAME} version. Set NESTR_VERSION explicitly." >&2; exit 1; }
    echo "Installing ${BINARY_NAME} v${version} (${target})…"

    tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT
    archive="nestr-${version}-${target}.tar.gz"
    base="https://github.com/${REPO}/releases/download/v${version}"
    curl -fsSL "${base}/${archive}" -o "${tmp}/${archive}"
    curl -fsSL "${base}/checksums-sha256.txt" -o "${tmp}/checksums.txt"

    expected="$(grep -F "$archive" "${tmp}/checksums.txt" | cut -d ' ' -f 1)"
    if command -v sha256sum >/dev/null 2>&1; then
        actual="$(sha256sum "${tmp}/${archive}" | cut -d ' ' -f 1)"
    else
        actual="$(shasum -a 256 "${tmp}/${archive}" | cut -d ' ' -f 1)"
    fi
    [ "$expected" = "$actual" ] || { echo "Checksum mismatch" >&2; exit 1; }

    tar xzf "${tmp}/${archive}" -C "$tmp"
    dir="${NESTR_INSTALL_DIR:-}"
    if [ -z "$dir" ]; then
        if [ -w "/usr/local/bin" ]; then dir="/usr/local/bin"; else mkdir -p "$HOME/.local/bin"; dir="$HOME/.local/bin"; fi
    fi
    if [ -w "$dir" ]; then
        cp "${tmp}/${BINARY_NAME}" "${dir}/"
        chmod +x "${dir}/${BINARY_NAME}"
    else
        sudo cp "${tmp}/${BINARY_NAME}" "${dir}/"
        sudo chmod +x "${dir}/${BINARY_NAME}"
    fi
    echo "Installed to ${dir}/${BINARY_NAME}. Run 'nestr --help'."
    case ":$PATH:" in *":${dir}:"*) ;; *) echo "Note: add ${dir} to your PATH." ;; esac
}

main "$@"
