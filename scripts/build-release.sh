#!/usr/bin/env bash
# Builds release binaries for both of fpx-convert's required targets
# (x86_64 Asustor NAS, aarch64 for ARM-based Asustor models/devices) and
# packages them into packaging/x86_64/ and packaging/arm64/ (the default
# delivery path for shipping binaries), alongside the spec that documents
# fpx-convert's behavior and CLI contract, and a BUILD_INFO.txt recording
# exactly what went into the build.
#
# Binaries are fully static (musl libc, no dynamic dependencies) so they
# don't depend on whatever glibc version the NAS firmware happens to ship.
# cargo-zigbuild cross-links them, using the Zig toolchain as a portable
# stand-in for per-target cross-compiler packages.
#
# Run from inside the devcontainer (or anywhere with the prerequisites
# below already set up) via: ./scripts/build-release.sh
#
# Prerequisites, if not using the devcontainer:
#   - rustup targets: x86_64-unknown-linux-musl, aarch64-unknown-linux-musl
#   - zig (see .devcontainer/Dockerfile for the pinned version/install steps)
#   - cargo-zigbuild: cargo install cargo-zigbuild

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Rust target triple -> short delivery directory name.
declare -A TARGETS=(
    [x86_64-unknown-linux-musl]=x86_64
    [aarch64-unknown-linux-musl]=arm64
)
PACKAGING_DIR="$REPO_ROOT/packaging"

rm -rf "$PACKAGING_DIR"
mkdir -p "$PACKAGING_DIR"

for target in "${!TARGETS[@]}"; do
    echo "==> Building $target"
    cargo zigbuild --release --target "$target"

    target_dir="$PACKAGING_DIR/${TARGETS[$target]}"
    mkdir -p "$target_dir"
    cp "target/$target/release/fpx-convert" "$target_dir/"
done

cp specs/0001-fpx-conversion-pipeline.md "$PACKAGING_DIR/"

VERSION="$(cargo metadata --no-deps --format-version 1 | grep -o '"version":"[^"]*"' | head -1 | cut -d'"' -f4)"
GIT_COMMIT="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
GIT_DIRTY=""
if ! git diff --quiet 2>/dev/null || ! git diff --cached --quiet 2>/dev/null; then
    GIT_DIRTY=" (with uncommitted changes)"
fi

cat > "$PACKAGING_DIR/BUILD_INFO.txt" <<EOF
fpx-convert $VERSION

Built:       $(date -u +"%Y-%m-%dT%H:%M:%SZ")
Git commit:  $GIT_COMMIT$GIT_DIRTY
Rustc:       $(rustc --version)
Targets:     x86_64-unknown-linux-musl (packaging/x86_64), aarch64-unknown-linux-musl (packaging/arm64)

Run any binary below with --help for full usage; see
0001-fpx-conversion-pipeline.md in this directory for the full behavioral
spec (CLI contract, exit codes, error handling, scope).
EOF

echo
echo "==> packaging/ contents:"
find "$PACKAGING_DIR" -type f -exec ls -lh {} \; | awk '{print $5, $9}'
