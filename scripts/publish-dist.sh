#!/usr/bin/env bash
# Publishes packaging/ (made by scripts/build-release.sh) as one commit on the orphan `dist`
# branch, tagged fpx-convert-<version>-<12-digit sha>, and pushes both (spec 0002).
#
# Run it where git can push (the host), after building on a clean `main` that matches
# origin/main: ./scripts/publish-dist.sh
# Publishing the same build again changes nothing. Written for bash 3.2 (macOS's /bin/bash).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"
PACKAGING="$REPO_ROOT/packaging"

die() { echo "publish-dist: $*" >&2; exit 1; }

[ "$(git symbolic-ref --short HEAD 2>/dev/null)" = main ] || die "not on main"
[ -z "$(git status --porcelain)" ] || die "the working tree is not clean"
git fetch --quiet origin main
head="$(git rev-parse HEAD)"
[ "$head" = "$(git rev-parse origin/main)" ] || die "main is not at origin/main"
[ -f "$PACKAGING/BUILD_INFO.txt" ] || die "no packaging/BUILD_INFO.txt: run scripts/build-release.sh"
built="$(sed -n 's/^commit  *//p' "$PACKAGING/BUILD_INFO.txt")"
[ "$built" = "$head" ] || die "packaging/ was built from ${built:-an unknown commit}, not $head"
version="$(sed -n '1s/^fpx-convert //p' "$PACKAGING/BUILD_INFO.txt")"
[ -n "$version" ] || die "BUILD_INFO.txt does not name a version"
tag="fpx-convert-$version-${head:0:12}"

# Build the tree in a scratch index, so the checkout and its index are never touched.
index="$(mktemp -d)/index"
trap 'rm -rf "$(dirname "$index")"' EXIT
(cd "$PACKAGING" && find . -type f ! -name .DS_Store | sed 's#^\./##' | LC_ALL=C sort |
    GIT_INDEX_FILE="$index" git --git-dir="$REPO_ROOT/.git" --work-tree=. update-index --add --stdin)
tree="$(GIT_INDEX_FILE="$index" git write-tree)"

parent=""
if git ls-remote --exit-code --heads origin dist >/dev/null; then
    git fetch --quiet origin dist
    parent="$(git rev-parse FETCH_HEAD)"
fi

if [ -n "$parent" ] && [ "$(git rev-parse "$parent^{tree}")" = "$tree" ]; then
    commit="$parent"
    echo "==> dist already holds this build ($commit)"
else
    message="fpx-convert $version (${head:0:12})

Release build of main at $head: binaries for x86-64 and arm64 and their
licence record (spec 0002). BUILD_INFO.txt names the toolchain and the
link evidence."
    if [ -n "$parent" ]; then
        commit="$(git commit-tree "$tree" -p "$parent" -m "$message")"
    else
        commit="$(git commit-tree "$tree" -m "$message")"
    fi
    echo "==> new dist commit $commit"
fi

if git ls-remote --exit-code --tags origin "refs/tags/$tag" >/dev/null; then
    git fetch --quiet origin "refs/tags/$tag:refs/tags/$tag"
fi
if git rev-parse --quiet --verify "refs/tags/$tag" >/dev/null; then
    [ "$(git rev-parse "$tag^{tree}")" = "$tree" ] ||
        die "tag $tag already exists with different content"
    commit="$(git rev-parse "$tag^{commit}")"
    echo "==> tag $tag already points at this build"
else
    git tag -a "$tag" -m "fpx-convert $version, built from $head" "$commit"
    echo "==> tagged $tag"
fi

git push origin "$commit:refs/heads/dist" "refs/tags/$tag"
echo "==> pushed dist ($commit) and $tag"
