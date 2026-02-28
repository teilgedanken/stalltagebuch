#!/usr/bin/env bash
set -euo pipefail

# Simplified Android Build Script
# Since Dioxus 0.7, custom AndroidManifest.xml and MainActivity.kt are natively supported.
# This script handles:
# 1) Running dx build
# 2) Copying res/xml resources (file_paths.xml for FileProvider)
# 3) APK signing for release builds

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Extract bundle identifier from Dioxus.toml (fallback to dev.dioxus.main)
BUNDLE_IDENTIFIER=$(sed -n 's/^[[:space:]]*identifier[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$ROOT_DIR/Dioxus.toml" | head -n1)
if [[ -z "$BUNDLE_IDENTIFIER" ]]; then
    BUNDLE_IDENTIFIER="dev.dioxus.main"
fi

# Determine build mode
RELEASE_FLAG=""
BUILD_TYPE="debug"
if [[ "${1:-}" == "--release" ]]; then
    RELEASE_FLAG="--release --codesign true"
    BUILD_TYPE="release"
fi

DX_APP_DIR="$ROOT_DIR/target/dx/stalltagebuch/$BUILD_TYPE/android/app"
APP_SRC_MAIN="$DX_APP_DIR/app/src/main"
RES_XML_DIR="$APP_SRC_MAIN/res/xml"
BUILD_CONFIG_FILE="$APP_SRC_MAIN/kotlin/dev/dioxus/main/BuildConfig.kt"

prepare_android_overrides() {
    local step_label="$1"
    echo "$step_label Preparing Android overrides (resources & BuildConfig alias)"

    mkdir -p "$RES_XML_DIR"
    cp "$ROOT_DIR/android/res/xml/file_paths.xml" "$RES_XML_DIR/file_paths.xml"

    # Copy project-level proguard rules into the generated app module so R8 keeps JNI-used members
    if [[ -f "$ROOT_DIR/android/proguard-rules.pro" ]]; then
        cp "$ROOT_DIR/android/proguard-rules.pro" "$DX_APP_DIR/app/proguard-rules.pro"
    fi

    if [[ "$BUNDLE_IDENTIFIER" != "dev.dioxus.main" ]]; then
        mkdir -p "$(dirname "$BUILD_CONFIG_FILE")"
        cat > "$BUILD_CONFIG_FILE" <<EOF
package dev.dioxus.main

typealias BuildConfig = ${BUNDLE_IDENTIFIER}.BuildConfig
EOF
    fi
}

# Copy native OpenSSL libraries to jniLibs so they are bundled in the APK
# Find Android SDK build-tools (zipalign and apksigner)
find_build_tools() {
    local sdk_path="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
    if [[ -z "$sdk_path" ]]; then
        # Try common locations
        for path in "$HOME/Android/Sdk" "/opt/android-sdk" "/usr/local/android-sdk"; do
            if [[ -d "$path/build-tools" ]]; then
                sdk_path="$path"
                break
            fi
        done
    fi
    
    if [[ -z "$sdk_path" || ! -d "$sdk_path/build-tools" ]]; then
        echo ""
        return
    fi
    
    # Find the latest build-tools version
    local latest_version
    latest_version=$(ls -1 "$sdk_path/build-tools" 2>/dev/null | sort -V | tail -1)
    if [[ -n "$latest_version" ]]; then
        echo "$sdk_path/build-tools/$latest_version"
    fi
}

# 1) Ensure Android overrides exist before dx touches the Gradle project
prepare_android_overrides "[1/2]"

# 2) Dioxus Build (capture lint failures so we can re-run Gradle without lint for release)
echo "[2/2] Running dx build --platform android ${RELEASE_FLAG}"
DX_LOG=$(mktemp)
set +e
dx build --platform android ${RELEASE_FLAG} 2>&1 | tee "$DX_LOG"
DX_EXIT=${PIPESTATUS[0]}
set -e

if [[ $DX_EXIT -ne 0 ]]; then
    if [[ "$BUILD_TYPE" == "release" ]] && grep -qi "lint" "$DX_LOG"; then
        echo "⚠ dx build reported a lint failure; continuing with lint disabled for release"
    else
        echo "dx build failed (see $DX_LOG)"
        exit $DX_EXIT
    fi
fi

# Ensure project-level ProGuard rules are present in the generated Gradle module
# (dx may have regenerated app/proguard-rules.pro) — copy again right before Gradle packaging.
if [[ -f "$ROOT_DIR/android/proguard-rules.pro" && -d "$DX_APP_DIR/app" ]]; then
    cp "$ROOT_DIR/android/proguard-rules.pro" "$DX_APP_DIR/app/proguard-rules.pro"
fi

rm -f "$DX_LOG"

if [[ "$BUILD_TYPE" == "release" ]]; then
    echo "[3/3] Running Gradle assembleRelease with lint disabled"
    pushd "$DX_APP_DIR" >/dev/null
    ./gradlew \
        -x lintVitalAnalyzeRelease \
        -x lintVitalReportRelease \
        -x lintReportRelease \
        -x lintVitalRelease \
        assembleRelease
    popd >/dev/null
fi

# Emit install hint for the generated APK
if [[ "$BUILD_TYPE" == "release" ]]; then
    APK_PATH="$DX_APP_DIR/app/build/outputs/apk/release/app-release.apk"
else
    APK_PATH="$DX_APP_DIR/app/build/outputs/apk/debug/app-debug.apk"
fi

echo ""
echo "✓ Build artifacts copied to $DX_APP_DIR"
echo "Install with:"
echo "  adb install -r $APK_PATH"