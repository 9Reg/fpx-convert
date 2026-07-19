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
**aarch64** (other, ARM-based Asustor models/devices). Build both with:

```
./scripts/build-release.sh
```

This cross-compiles a release binary for each target as a fully static
musl binary (no `libc.so`/`ld-linux` dependency — runs regardless of
whatever glibc, or lack of one, the NAS firmware ships), strips debug
symbols, and packages everything into `dist/` (gitignored — rebuild
locally rather than committing binaries):

```
dist/
  x86_64-unknown-linux-musl/fpx-convert
  aarch64-unknown-linux-musl/fpx-convert
  0001-fpx-conversion-pipeline.md   # the behavioral spec, bundled so it
                                     # travels with the binaries even if
                                     # only dist/ is copied elsewhere
  BUILD_INFO.txt                    # version, git commit, rustc version,
                                     # build timestamp
```

### Building outside the devcontainer

If you're not using `.devcontainer/`, you'll need to source these
yourself (see `.devcontainer/Dockerfile` for the exact version/install
steps):

- Rust targets: `rustup target add x86_64-unknown-linux-musl aarch64-unknown-linux-musl`
- [Zig](https://ziglang.org/download/) (pinned version in the Dockerfile),
  on your `PATH`
- `cargo-zigbuild`: `cargo install cargo-zigbuild`

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
