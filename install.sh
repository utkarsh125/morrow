#!/usr/bin/env bash
set -e

REPO="utkarsh125/morrow"
INSTALL_DIR="${HOME}/.local/bin"

mkdir -p "$INSTALL_DIR"

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$OS" in
  linux)
    case "$ARCH" in
      x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
      aarch64|arm64) TARGET="aarch64-unknown-linux-gnu" ;;
      *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
    esac
    ;;
  darwin)
    case "$ARCH" in
      x86_64) TARGET="x86_64-apple-darwin" ;;
      arm64|aarch64) TARGET="aarch64-apple-darwin" ;;
      *) echo "Unsupported architecture: $ARCH" && exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS. Please install with cargo: cargo install --git https://github.com/${REPO}"
    exit 1
    ;;
esac

LATEST_RELEASE=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
  echo "Failed to fetch latest release tag. Falling back to building from source..."
  cargo install --git "https://github.com/${REPO}"
  exit 0
fi

ARCHIVE_NAME="morrow-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_RELEASE}/${ARCHIVE_NAME}"

echo "Downloading Morrow ${LATEST_RELEASE} for ${TARGET}..."
TMP_DIR=$(mktemp -d)
curl -sL "$DOWNLOAD_URL" -o "${TMP_DIR}/${ARCHIVE_NAME}"

tar -xzf "${TMP_DIR}/${ARCHIVE_NAME}" -C "$TMP_DIR"
chmod +x "${TMP_DIR}/morrow"
mv "${TMP_DIR}/morrow" "${INSTALL_DIR}/morrow"
rm -rf "$TMP_DIR"

echo "Morrow installed successfully to ${INSTALL_DIR}/morrow!"

if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
  echo ""
  echo "Note: Make sure ${INSTALL_DIR} is in your PATH. Add this to your shell config (~/.bashrc, ~/.zshrc):"
  echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
