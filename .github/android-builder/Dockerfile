# =============================================================
# Android + Rust + dx CLI builder image for stalltagebuch
#
# Rebuild this image (Actions → docker-builder → Run workflow)
# when any of these change:
#   - DX_VERSION            (dioxus-cli version)
#   - NDK_VERSION           (Android NDK)
#   - ANDROID_API           (min SDK / NDK clang wrapper suffix)
#   - ANDROID_COMPILE_SDK   (Android platform)
#   - ANDROID_BUILD_TOOLS   (Android build-tools)
#   - Rust toolchain (stable bumps handled automatically)
# =============================================================

# eclipse-temurin:17-jdk-noble = Ubuntu 24.04 ("noble") + Temurin JDK 17.
# Java is already installed at /opt/java/openjdk — no Adoptium APT repo needed.
FROM eclipse-temurin:17-jdk-noble

ENV DEBIAN_FRONTEND=noninteractive

# ── Pinned versions ─────────────────────────────────────────────
ARG DX_VERSION=0.7.9
ARG NDK_VERSION=27.2.12479018
ARG ANDROID_API=28
ARG ANDROID_COMPILE_SDK=34
ARG ANDROID_BUILD_TOOLS=34.0.0

# ── Environment ───────────────────────────────────────────────
# JAVA_HOME is set by the base image; we redeclare it so downstream
# scripts that source this image can rely on it being documented here.
ENV JAVA_HOME=/opt/java/openjdk
ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_SDK_ROOT=/opt/android-sdk
ENV ANDROID_NDK_HOME=/opt/android-sdk/ndk/${NDK_VERSION}
# ANDROID_NDK_ROOT is the alias preferred by some crates (openssl-sys, ring, …)
ENV ANDROID_NDK_ROOT=/opt/android-sdk/ndk/${NDK_VERSION}
ENV CARGO_HOME=/root/.cargo
ENV RUSTUP_HOME=/root/.rustup
ENV PATH="${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${CARGO_HOME}/bin:${PATH}"

# ── Cargo / cc-crate / ar cross-compilation env for Android targets ─────
# These are ENV (not ARG) so they persist into every container run from this
# image and override the [target.*].linker and [env].CC_*/AR_* entries that
# may be hardcoded to local dev machine paths in .cargo/config.toml.
# NDK_TOOLCHAIN is set first so subsequent lines can reference it.
ENV NDK_TOOLCHAIN=${ANDROID_NDK_HOME}/toolchains/llvm/prebuilt/linux-x86_64/bin
ENV CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=${NDK_TOOLCHAIN}/aarch64-linux-android${ANDROID_API}-clang \
    CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER=${NDK_TOOLCHAIN}/armv7a-linux-androideabi${ANDROID_API}-clang \
    CARGO_TARGET_I686_LINUX_ANDROID_LINKER=${NDK_TOOLCHAIN}/i686-linux-android${ANDROID_API}-clang \
    CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=${NDK_TOOLCHAIN}/x86_64-linux-android${ANDROID_API}-clang \
    CC_aarch64_linux_android=${NDK_TOOLCHAIN}/aarch64-linux-android${ANDROID_API}-clang \
    CC_armv7_linux_androideabi=${NDK_TOOLCHAIN}/armv7a-linux-androideabi${ANDROID_API}-clang \
    CC_i686_linux_android=${NDK_TOOLCHAIN}/i686-linux-android${ANDROID_API}-clang \
    CC_x86_64_linux_android=${NDK_TOOLCHAIN}/x86_64-linux-android${ANDROID_API}-clang \
    AR_aarch64_linux_android=${NDK_TOOLCHAIN}/llvm-ar \
    AR_armv7_linux_androideabi=${NDK_TOOLCHAIN}/llvm-ar \
    AR_i686_linux_android=${NDK_TOOLCHAIN}/llvm-ar \
    AR_x86_64_linux_android=${NDK_TOOLCHAIN}/llvm-ar

# ── System packages ───────────────────────────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
      curl \
      unzip \
      git \
      ca-certificates \
      # Required by the vendored OpenSSL build in Cargo.toml
      pkg-config \
      libssl-dev \
      # Required by the Rust linker for Android cross-compilation
      gcc \
      # C runtime startup files (Scrt1.o, crti.o) and linker stubs (-lc, -lpthread…)
      # needed when Cargo compiles build.rs scripts for the host (x86_64-unknown-linux-gnu).
      # Technically a transitive dep of gcc, but the temurin base suppresses it.
      libc6-dev \
      # Required by the vendored OpenSSL build (openssl crate, features=["vendored"])
      make \
    && rm -rf /var/lib/apt/lists/*

# ── Android SDK command-line tools ────────────────────────────
# Pinned cmdline-tools build (11076708 = latest as of 2025-Q2).
# Update the zip filename if Google releases a newer revision.
RUN mkdir -p "${ANDROID_HOME}/cmdline-tools" \
    && curl -fsSL \
         https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip \
         -o /tmp/cmdtools.zip \
    && unzip -q /tmp/cmdtools.zip -d "${ANDROID_HOME}/cmdline-tools" \
    && mv "${ANDROID_HOME}/cmdline-tools/cmdline-tools" \
         "${ANDROID_HOME}/cmdline-tools/latest" \
    && rm /tmp/cmdtools.zip \
    # Accept all licences non-interactively
    && yes | sdkmanager --licenses > /dev/null \
    && sdkmanager \
         "platform-tools" \
         "platforms;android-${ANDROID_COMPILE_SDK}" \
         "build-tools;${ANDROID_BUILD_TOOLS}" \
         "ndk;${NDK_VERSION}"

# ── Rust stable + Android cross-compilation targets ───────────
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y \
           --default-toolchain stable \
           --no-modify-path \
           --profile minimal \
    && rustup target add \
         aarch64-linux-android \
         armv7-linux-androideabi \
         x86_64-linux-android

# ── dioxus-cli (dx) ───────────────────────────────────────────
# Download the pre-built Linux binary directly from the GitHub release.
# To update: bump DX_VERSION above and re-run the docker-builder workflow.
RUN curl -fsSL \
      "https://github.com/DioxusLabs/dioxus/releases/download/v${DX_VERSION}/dx-x86_64-unknown-linux-gnu.tar.gz" \
      | tar -xz -C /usr/local/bin \
    && chmod +x /usr/local/bin/dx

WORKDIR /build
