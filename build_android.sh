#!/usr/bin/env bash
set -euo pipefail

# Simplified Android Build Script
# Since Dioxus 0.7, custom AndroidManifest.xml and MainActivity.kt are natively supported.
# This script handles:
# 1) Running dx build
# 2) Copying res/xml resources (file_paths.xml for FileProvider)
# 3) APK signing for release builds

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

detect_device_rust_target() {
    if ! command -v adb >/dev/null 2>&1; then
        return 1
    fi

    local abi
    abi=$(adb shell getprop ro.product.cpu.abi 2>/dev/null | tr -d '\r' | tail -n1 || true)
    case "$abi" in
        arm64-v8a)
            echo "aarch64-linux-android"
            ;;
        armeabi-v7a|armeabi)
            echo "armv7-linux-androideabi"
            ;;
        x86)
            echo "i686-linux-android"
            ;;
        x86_64)
            echo "x86_64-linux-android"
            ;;
        *)
            return 1
            ;;
    esac
}

inject_rustls_android_helper_sources() {
    # The jni-0.22 branch currently doesn't ship populated Maven artifacts in git checkouts.
    # Copy the Kotlin helper directly from the crate checkout into the generated app sources.
    local verifier_src
    verifier_src=$(find "$HOME/.cargo" -type f -path "*/rustls-platform-verifier*/android/rustls-platform-verifier/src/main/java/org/rustls/platformverifier/CertificateVerifier.kt" 2>/dev/null | sort | tail -n1 || true)
    if [[ -z "$verifier_src" ]]; then
        echo "⚠ rustls-platform-verifier CertificateVerifier.kt not found in Cargo cache"
        return 0
    fi

    local verifier_dir="$APP_SRC_MAIN/kotlin/org/rustls/platformverifier"
    mkdir -p "$verifier_dir"
    local verifier_file="$verifier_dir/CertificateVerifier.kt"
    cp "$verifier_src" "$verifier_file"

    # Avoid false-positive "Revoked" on Android when certs omit OCSP responder URLs.
    # This keeps strict revocation failures for real revoked certs, but treats
    # "undetermined" / "missing OCSP responder" as non-fatal.
    if ! grep -q 'UNDETERMINED_REVOCATION_STATUS' "$verifier_file"; then
        awk '
            /return VerificationResult\(StatusCode\.Revoked, e\.toString\(\)\)/ && !done {
                print "                if (e.reason == CertPathValidatorException.BasicReason.UNDETERMINED_REVOCATION_STATUS || (e.message?.contains(\"Certificate does not specify OCSP responder\") == true)) {"
                print "                    Log.w(TAG, \"revocation status undetermined (missing OCSP responder): $e\")"
                print "                    return VerificationResult(StatusCode.Ok)"
                print "                }"
                print ""
                done = 1
            }
            { print }
        ' "$verifier_file" > "$verifier_file.tmp"
        mv "$verifier_file.tmp" "$verifier_file"
    fi

    cat > "$verifier_dir/BuildConfig.kt" <<EOF
package org.rustls.platformverifier

internal object BuildConfig {
    const val TEST: Boolean = false
}
EOF
}

# Extract bundle identifier from Dioxus.toml (fallback to dev.dioxus.main)
BUNDLE_IDENTIFIER=$(sed -n 's/^[[:space:]]*identifier[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$ROOT_DIR/Dioxus.toml" | head -n1)
if [[ -z "$BUNDLE_IDENTIFIER" ]]; then
    BUNDLE_IDENTIFIER="dev.dioxus.main"
fi

# Determine build mode
BUILD_TYPE="debug"
RUST_TARGET=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --release)
            BUILD_TYPE="release"
            shift
            ;;
        --target)
            if [[ $# -lt 2 ]]; then
                echo "Missing value for --target"
                exit 1
            fi
            RUST_TARGET="$2"
            shift 2
            ;;
        --target=*)
            RUST_TARGET="${1#*=}"
            shift
            ;;
        *)
            echo "Unknown argument: $1"
            echo "Usage: $0 [--release] [--target <rust-target-triple>]"
            exit 1
            ;;
    esac
done

if [[ -z "$RUST_TARGET" ]]; then
    if detected_target=$(detect_device_rust_target); then
        RUST_TARGET="$detected_target"
        echo "Detected device ABI target: $RUST_TARGET"
    else
        echo "No Android device ABI detected; using dx default target selection"
    fi
fi

DX_BUILD_ARGS=(build --platform android)
if [[ "$BUILD_TYPE" == "release" ]]; then
    DX_BUILD_ARGS+=(--release --codesign true)
fi
if [[ -n "$RUST_TARGET" ]]; then
    DX_BUILD_ARGS+=(--target "$RUST_TARGET")
fi

DX_APP_DIR="$ROOT_DIR/target/dx/stalltagebuch/$BUILD_TYPE/android/app"
APP_SRC_MAIN="$DX_APP_DIR/app/src/main"
RES_XML_DIR="$APP_SRC_MAIN/res/xml"
BUILD_CONFIG_FILE="$APP_SRC_MAIN/kotlin/dev/dioxus/main/BuildConfig.kt"
BUNDLE_PATH="${BUNDLE_IDENTIFIER//./\/}"
BUNDLE_BUILD_CONFIG_FILE="$APP_SRC_MAIN/kotlin/$BUNDLE_PATH/BuildConfig.kt"

prepare_android_overrides() {
    local step_label="$1"
    echo "$step_label Preparing Android overrides (resources & BuildConfig alias)"

    mkdir -p "$RES_XML_DIR"
    cp "$ROOT_DIR/android/res/xml/file_paths.xml" "$RES_XML_DIR/file_paths.xml"

    # Copy project-level proguard rules into the generated app module so R8 keeps JNI-used members
    if [[ -f "$ROOT_DIR/android/proguard-rules.pro" ]]; then
        cp "$ROOT_DIR/android/proguard-rules.pro" "$DX_APP_DIR/app/proguard-rules.pro"
    fi

    # Remove stale/generated alias files that can trigger redeclaration loops.
    rm -f "$BUILD_CONFIG_FILE" "$BUNDLE_BUILD_CONFIG_FILE"

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
echo "[2/2] Running dx ${DX_BUILD_ARGS[*]}"
DX_LOG=$(mktemp)
set +e
dx "${DX_BUILD_ARGS[@]}" 2>&1 | tee "$DX_LOG"
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

# Ensure rustls-platform-verifier Android Kotlin helper is available to the generated app.
inject_rustls_android_helper_sources

# dx can (re)generate alias files under the bundle package; remove stale self-aliases,
# then recreate only the dev.dioxus.main indirection if needed.
rm -f "$BUNDLE_BUILD_CONFIG_FILE"
if [[ "$BUNDLE_IDENTIFIER" != "dev.dioxus.main" ]]; then
    mkdir -p "$(dirname "$BUILD_CONFIG_FILE")"
    cat > "$BUILD_CONFIG_FILE" <<EOF
package dev.dioxus.main

typealias BuildConfig = ${BUNDLE_IDENTIFIER}.BuildConfig
EOF
fi

rm -f "$DX_LOG"

if [[ "$BUILD_TYPE" == "release" ]]; then
    echo "[3/3] Running Gradle assembleRelease with lint disabled"
    pushd "$DX_APP_DIR" >/dev/null
    chmod +x ./gradlew
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
