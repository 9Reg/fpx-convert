# fpx-convert

Converts a FlashPix (`.fpx`) image into a PNG (lossless, the default) or
JPEG (lossy, opt-in) a modern web browser can display directly. Camera
model and capture date, if present in the source file, are preserved in
the output as EXIF (a PNG `eXIf` chunk, or a JPEG APP1 `Exif` segment).

One file in, one file out, per invocation — see
[specs/0001-fpx-conversion-pipeline.md](specs/0001-fpx-conversion-pipeline.md)
for the full behavioral spec (CLI contract, exit codes, error handling,
scope, and why it's shaped the way it is). The compiled binary is also
self-documenting: run it with `--help`.

Built to support [Lumento](https://github.com/9Reg) (a separate project,
written in Go) as a subprocess, but fpx-convert is a standalone tool usable
by anything that can run a binary and read stdin/stdout or a file path.

## Usage

```
fpx-convert <input.fpx> <output.png>
fpx-convert --format jpeg <input.fpx> <output.jpg>
fpx-convert --stdin --stdout
fpx-convert --stdin --stdout --format jpeg
fpx-convert --help
```

## Developing

This repo's `.devcontainer/` sets up everything needed: a Rust toolchain,
both required cross-compilation targets, and Zig (used via `cargo-zigbuild`
as the cross-linker for fully static musl binaries). Open the repo in the
devcontainer, then:

```
cargo build          # debug build for the host architecture
cargo test            # unit + integration tests
cargo clippy --all-targets -- -D warnings
```

`test-media/` can hold local `.fpx` sample files for manual testing —
that directory is gitignored, so nothing there gets committed.

## Building a release (both targets)

fpx-convert must run on two architectures: **x86_64** (Asustor NAS) and
**aarch64** (other, ARM-based Asustor models/devices). A release is built
inside the devcontainer, from a clean, committed checkout:

```
./scripts/build-release.sh
```

This cross-compiles a fully static musl binary for each target (no
`libc.so`/`ld-linux` dependency, so it runs whatever libc the NAS firmware
ships) and builds the **licence record** that
[spec 0002](specs/0002-release-licence-record.md) defines: every component
linked into each binary, its licence and licence texts, and its source.
The record is derived from the real link, not assumed — the build replays
zig's final link command with a linker map, requires the replay to be
byte-identical to the shipped binary, and fails if the linker is given
anything the record does not cover, or if any licence lacks its text.
Every input the linker is given gets a line, even one that ends up
contributing 0 bytes; `BUILD_INFO.txt` shows what each contributed.
Everything lands in `packaging/` (gitignored):

```
packaging/
  fpx-convert/x86-64/fpx-convert           # x86_64-unknown-linux-musl
  fpx-convert/arm64/fpx-convert            # aarch64-unknown-linux-musl
  third-party/fpx-convert-<arch>/
    entries.tsv                            # one line per component, 6 fields
    licenses/                              # every licence text
    source/                                # fpx-convert's own source tarball
  BUILD_INFO.txt                           # commit, toolchain, hashes,
                                           # link evidence
```

Building the same commit twice gives byte-identical files. The toolchain
is pinned (`rust-toolchain.toml`; zig and cargo-zigbuild in
`.devcontainer/Dockerfile`) and the build refuses any other version,
because the record's upstream source URLs are tied to those versions.

### Publishing to the `dist` branch

Releases are published as commits on the orphan `dist` branch (never
merged into `main`), which Lumento fetches. After building on `main`, on
the host where git can push:

```
./scripts/publish-dist.sh
```

It refuses anything but a clean `main` that matches `origin/main` and a
`packaging/` built from that commit, commits `packaging/` to `dist`, tags
it `fpx-convert-<version>-<12-digit sha>` and pushes both. Running it
again for the same build changes nothing.

### Building outside the devcontainer

Releases need the devcontainer's pinned toolchain. For ordinary
cross-builds elsewhere, see `.devcontainer/Dockerfile` for the exact
versions:

- Rust: `rustup` installs the pinned toolchain and both musl targets from
  `rust-toolchain.toml` on first use
- [Zig](https://ziglang.org/download/) (pinned version in the Dockerfile),
  on your `PATH`
- `cargo-zigbuild` (pinned version in the Dockerfile):
  `cargo install --locked cargo-zigbuild@<version>`

No `.cargo/config.toml` linker config is needed — `cargo zigbuild` handles
cross-linking for each target itself.

## References

FlashPix is an obscure, largely undocumented-outside-primary-sources 1990s
format. These were the sources consulted while building the parser:

- [Original 1996 Kodak FlashPix spec](http://graphcomp.com/info/specs/livepicture/fpx.pdf)
- [W3C-hosted official FPX spec](https://www.w3.org/Graphics/FlashPix/FPX-spec.pdf)
- [FlashPix architecture white paper](https://www.w3.org/Graphics/FlashPix/FPwhite.pdf)
- [`libfpx`](https://github.com/ImageMagick/libfpx) — the original
  Kodak/Digital Imaging Group reference implementation (1999, maintained
  today as a courtesy by ImageMagick Studio LLC). Read as documentation to
  confirm exact byte-level details the primary spec text doesn't restate
  (property IDs, struct layouts, tile-table format) — no `libfpx` source
  was copied, ported, or linked into fpx-convert; only factual information
  about the file format itself was derived from it. `libfpx`'s own license
  (see `flashpix.h` in that repo) is a permissive, Apache-1.0-like license
  from Digital Imaging Group Inc. and Eastman Kodak Company.

## License

AGPL-3.0 — see [LICENSE](LICENSE).
