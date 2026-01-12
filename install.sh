#!/bin/sh
set -e

REPO="davidhariri/mixyt"
INSTALL_DIR="/usr/local/bin"

# Detect architecture
ARCH=$(uname -m)
case "$ARCH" in
    x86_64)  ARTIFACT="mixyt-macos-x86_64" ;;
    arm64)   ARTIFACT="mixyt-macos-aarch64" ;;
    aarch64) ARTIFACT="mixyt-macos-aarch64" ;;
    *)       echo "Unsupported architecture: $ARCH"; exit 1 ;;
esac

# Check for macOS
OS=$(uname -s)
if [ "$OS" != "Darwin" ]; then
    echo "mixyt only supports macOS"
    exit 1
fi

echo "Installing mixyt for $ARCH..."

# Stop daemon if running (for upgrades)
if command -v mixyt >/dev/null 2>&1; then
    echo "Stopping existing daemon..."
    mixyt daemon stop 2>/dev/null || true
fi

# Get latest release URL
DOWNLOAD_URL="https://github.com/$REPO/releases/latest/download/$ARTIFACT"

# Download and install
TMP_FILE=$(mktemp)
curl -fsSL "$DOWNLOAD_URL" -o "$TMP_FILE"
chmod +x "$TMP_FILE"

# Move to install directory (may need sudo)
if [ -w "$INSTALL_DIR" ]; then
    mv "$TMP_FILE" "$INSTALL_DIR/mixyt"
else
    echo "Need sudo to install to $INSTALL_DIR"
    sudo mv "$TMP_FILE" "$INSTALL_DIR/mixyt"
fi

echo "mixyt installed to $INSTALL_DIR/mixyt"
echo ""
echo "Make sure you have the dependencies:"
echo "  brew install yt-dlp ffmpeg"
echo ""
echo "Get started:"
echo "  mixyt add <youtube-url>"
echo "  mixyt play <search>"
