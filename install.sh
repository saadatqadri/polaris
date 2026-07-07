#!/bin/sh
# Polaris installer: fetches the latest prebuilt binary from GitHub Releases.
#   curl -fsSL https://raw.githubusercontent.com/saadatqadri/polaris/main/install.sh | sh
set -eu

REPO="saadatqadri/polaris"

case "$(uname -s)" in
  Darwin) os="apple-darwin" ;;
  Linux) os="unknown-linux-gnu" ;;
  *) echo "polaris: unsupported OS ($(uname -s)) — build from source with cargo" >&2; exit 1 ;;
esac

case "$(uname -m)" in
  arm64 | aarch64) arch="aarch64" ;;
  x86_64) arch="x86_64" ;;
  *) echo "polaris: unsupported architecture ($(uname -m))" >&2; exit 1 ;;
esac

if [ "$os" = "unknown-linux-gnu" ] && [ "$arch" = "aarch64" ]; then
  echo "polaris: no Linux arm64 build yet — build from source with cargo" >&2
  exit 1
fi

target="${arch}-${os}"
url="https://github.com/${REPO}/releases/latest/download/polaris-${target}.tar.gz"
dir="${POLARIS_INSTALL_DIR:-$HOME/.local/bin}"

mkdir -p "$dir"
echo "Downloading polaris (${target})…"
curl -fL --progress-bar "$url" | tar xz -C "$dir"
chmod +x "$dir/polaris"
echo "✓ Installed: $dir/polaris"

case ":$PATH:" in
  *":$dir:"*) ;;
  *)
    echo ""
    echo "  $dir is not on your PATH. Add this to your shell profile:"
    echo "    export PATH=\"$dir:\$PATH\""
    ;;
esac

echo ""
echo "Get writing:  polaris draft.md"
