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

# Download to a temp file first: a pipe into tar swallows curl failures
# (empty input can exit 0), which would turn a 404 into a confusing mess.
tmp="$(mktemp -t polaris.XXXXXX.tar.gz)"
trap 'rm -f "$tmp"' EXIT

echo "Downloading polaris (${target})…"
if ! curl -fL --progress-bar "$url" -o "$tmp"; then
  echo "" >&2
  echo "polaris: download failed ($url)" >&2
  echo "  Either no release has been published yet (check" >&2
  echo "  https://github.com/${REPO}/releases) or this target has no build." >&2
  exit 1
fi

mkdir -p "$dir"
tar xzf "$tmp" -C "$dir"
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
