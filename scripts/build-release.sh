#!/usr/bin/env bash
# Builds a release of fpx-convert for both required targets (x86_64 Asustor NAS, aarch64 for
# ARM-based models) and its licence record (spec 0002), into packaging/ in the layout of the
# `dist` branch:
#
#   packaging/fpx-convert/<arch>/fpx-convert
#   packaging/third-party/fpx-convert-<arch>/{entries.tsv,licenses/,source/}
#   packaging/BUILD_INFO.txt
#
# <arch> is x86-64 or arm64. Binaries are fully static (musl, no dynamic dependencies), so they
# don't depend on whatever libc the NAS firmware ships. cargo-zigbuild cross-links them with Zig.
#
# Run it inside the devcontainer, where the pinned toolchain is: ./scripts/build-release.sh
# It refuses a dirty working tree (a release is built from a commit) and a toolchain that differs
# from the pins (rust-toolchain.toml, .devcontainer/Dockerfile). scripts/publish-dist.sh then
# commits packaging/ to the `dist` branch, from the host.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# The bind-mounted checkout is owned by the host user; let git read it without changing any
# global config.
export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0="$REPO_ROOT"

if [ -n "$(git status --porcelain)" ]; then
    echo "build-release: the working tree is not clean; commit or stash first" >&2
    exit 1
fi

# jpeg-encoder's simd feature compiles a libjpeg-turbo port whose notice the record does not
# carry (spec 0002, "Proving what the final link used", 6): refuse it before building.
for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
    if ! features="$(cargo tree --quiet --locked -e features -i jpeg-encoder --target "$target")"; then
        echo "build-release: cannot resolve jpeg-encoder's features for $target" >&2
        exit 1
    fi
    if grep -q 'jpeg-encoder feature "simd"' <<<"$features"; then
        echo "build-release: jpeg-encoder's simd feature is enabled for $target; the licence" >&2
        echo "record lacks libjpeg-turbo's notice for src/avx2/fdct.rs (spec 0002)" >&2
        exit 1
    fi
done

# Rust target triple -> lumento's architecture name.
declare -A TARGETS=(
    [x86_64-unknown-linux-musl]=x86-64
    [aarch64-unknown-linux-musl]=arm64
)

# A fresh target directory per release: the link evidence comes from this build's own log, so
# nothing may be reused from an earlier one.
BUILD_DIR="$REPO_ROOT/target/release-build"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"

record_args=()
for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
    arch="${TARGETS[$target]}"
    target_dir="$BUILD_DIR/$arch"
    log="$BUILD_DIR/$arch.log"
    echo "==> Building $target"
    # -Csave-temps keeps the objects zig's ld.lld reads, so the link can be replayed with a map;
    # ZIG_VERBOSE_LINK=1 prints that ld.lld command. Neither changes the binary.
    if ! CARGO_TARGET_DIR="$target_dir" RUSTFLAGS=-Csave-temps ZIG_VERBOSE_LINK=1 \
        cargo zigbuild --locked --release --target "$target" >"$log" 2>&1; then
        tail -n 40 "$log" >&2
        exit 1
    fi
    record_args+=("$target:$arch:$target_dir:$log")
done

echo "==> Building the licence record"
python3 "$REPO_ROOT/scripts/licence-record.py" "$REPO_ROOT/packaging" "${record_args[@]}"

echo
echo "==> packaging/:"
cat "$REPO_ROOT/packaging/BUILD_INFO.txt"
