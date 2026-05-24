# =============================================================
# Android + Rust + dx CLI builder image for stalltagebuch
#
# Rebuild this image (Actions → docker-builder → Run workflow)
# when any of these change:
#   - DX_VERSION            (dioxus-cli version)
#   - NDK_VERSION           (Android NDK)
#   - ANDROID_COMPILE_SDK   (Android platform)
#   - ANDROID_BUILD_TOOLS   (Android build-tools)
#   - Rust toolchain (stable bumps handled automatically)
# =============================================================

# eclipse-temurin:17-jdk-noble = Ubuntu 24.04 ("noble") + Temurin JDK 17.
# Java is already installed at /opt/java/openjdk — no Adoptium APT repo needed.
FROM eclipse-temurin:17-jdk-noble

ENV DEBIAN_FRONTEND=noninteractive

# ── Pinned versions ───────────────────────────────────────────
ARG DX_VERSION=0.7.9
ARG NDK_VERSION=27.2.12479018
ARG ANDROID_COMPILE_SDK=34
ARG ANDROID_BUILD_TOOLS=34.0.0

# ── Environment ───────────────────────────────────────────────
# JAVA_HOME is set by the base image; we redeclare it so downstream
# scripts that source this image can rely on it being documented here.
ENV JAVA_HOME=/opt/java/openjdk
ENV ANDROID_HOME=/opt/android-sdk
ENV ANDROID_SDK_ROOT=/opt/android-sdk
ENV ANDROID_NDK_HOME=/opt/android-sdk/ndk/${NDK_VERSION}
ENV CARGO_HOME=/root/.cargo
ENV RUSTUP_HOME=/root/.rustup
ENV PATH="${JAVA_HOME}/bin:${ANDROID_HOME}/cmdline-tools/latest/bin:${ANDROID_HOME}/platform-tools:${CARGO_HOME}/bin:${PATH}"

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
