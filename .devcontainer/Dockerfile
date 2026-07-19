FROM mcr.microsoft.com/devcontainers/rust:1-bookworm

# Zig doubles as a cross-linker for Rust's musl targets (via cargo-zigbuild
# below) — no separate cross-toolchain packages needed per target arch.
# Version is pinned for reproducible builds; bump deliberately.
ARG ZIG_VERSION=0.13.0
RUN case "$(dpkg --print-architecture)" in \
        amd64) ZIG_ARCH=x86_64 ;; \
        arm64) ZIG_ARCH=aarch64 ;; \
        *) echo "unsupported host architecture for zig" >&2; exit 1 ;; \
    esac \
    && curl -fsSL -o /tmp/zig.tar.xz \
        "https://ziglang.org/download/${ZIG_VERSION}/zig-linux-${ZIG_ARCH}-${ZIG_VERSION}.tar.xz" \
    && mkdir -p /usr/local/zig \
    && tar -xJf /tmp/zig.tar.xz --strip-components=1 -C /usr/local/zig \
    && rm /tmp/zig.tar.xz \
    && ln -s /usr/local/zig/zig /usr/local/bin/zig

USER vscode

# fpx-convert's two required build targets (both 64-bit), built as fully
# static musl binaries so the shipped binary doesn't depend on whatever
# glibc the NAS firmware happens to ship:
#   - x86_64 (Asustor NAS)
#   - aarch64 (other, ARM-based Asustor models/devices)
RUN rustup target add \
        x86_64-unknown-linux-musl \
        aarch64-unknown-linux-musl

RUN cargo install cargo-zigbuild
