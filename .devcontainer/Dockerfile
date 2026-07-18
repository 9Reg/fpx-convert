FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Cross-toolchains for fpx-convert's two required build targets (both 64-bit):
#   - x86_64 (Asustor NAS)
#   - aarch64 (other, ARM-based Asustor models/devices)
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get install -y --no-install-recommends \
        gcc-x86-64-linux-gnu \
        libc6-dev-amd64-cross \
        gcc-aarch64-linux-gnu \
        libc6-dev-arm64-cross \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

USER vscode

RUN rustup target add \
        x86_64-unknown-linux-gnu \
        aarch64-unknown-linux-gnu
