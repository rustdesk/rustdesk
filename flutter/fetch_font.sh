#!/usr/bin/env bash
# Downloads JetBrainsMono Nerd Font for terminal glyph support (yazi, etc.)
set -euo pipefail

FONT_DIR="$(dirname "$0")/assets"
FONT_FILE="$FONT_DIR/JetBrainsMonoNerdFont-Regular.ttf"
FONT_URL="https://github.com/ryanoasis/nerd-fonts/releases/download/v3.3.0/JetBrainsMono.zip"

if [ -f "$FONT_FILE" ]; then
  echo "Font already exists: $FONT_FILE"
  exit 0
fi

echo "Downloading JetBrainsMono Nerd Font..."
mkdir -p "$FONT_DIR"
TMPZIP=$(mktemp /tmp/JetBrainsMono.XXXXXX.zip)
trap 'rm -f "$TMPZIP"' EXIT

wget -q "$FONT_URL" -O "$TMPZIP"
unzip -q -o "$TMPZIP" "JetBrainsMonoNerdFont-Regular.ttf" -d /tmp/
mv /tmp/JetBrainsMonoNerdFont-Regular.ttf "$FONT_FILE"
echo "Installed: $FONT_FILE ($(du -h "$FONT_FILE" | cut -f1))"
