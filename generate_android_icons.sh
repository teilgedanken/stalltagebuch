#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SVG_PATH="${1:-$ROOT_DIR/assets/logo.svg}"

if ! command -v inkscape >/dev/null 2>&1; then
    echo "Error: inkscape is required to generate Android icons." >&2
    exit 1
fi

if [[ ! -f "$SVG_PATH" ]]; then
    echo "Error: SVG not found: $SVG_PATH" >&2
    exit 1
fi

generate_png() {
    local size="$1"
    local output_path="$2"

    mkdir -p "$(dirname "$output_path")"
    inkscape "$SVG_PATH" \
        --export-type=png \
        --export-filename="$output_path" \
        --export-width="$size" \
        --export-height="$size" >/dev/null
}

generate_png 48 "$ROOT_DIR/android/res/mipmap-mdpi/stalltagebuch_launcher.png"
generate_png 72 "$ROOT_DIR/android/res/mipmap-hdpi/stalltagebuch_launcher.png"
generate_png 96 "$ROOT_DIR/android/res/mipmap-xhdpi/stalltagebuch_launcher.png"
generate_png 144 "$ROOT_DIR/android/res/mipmap-xxhdpi/stalltagebuch_launcher.png"
generate_png 192 "$ROOT_DIR/android/res/mipmap-xxxhdpi/stalltagebuch_launcher.png"
generate_png 432 "$ROOT_DIR/android/res/drawable-v24/stalltagebuch_launcher_foreground.png"

echo "Generated Android launcher icons from $SVG_PATH"
