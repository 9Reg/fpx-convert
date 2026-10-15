# 0002 — Release licence record and the `dist` branch

**Status:** Accepted (2026-10-15)

## Why

Lumento ships fpx-convert's binaries in its App Central package. Before a public release it must
name, for each architecture, everything compiled into the binary, under which licence, with the
licence texts and the source (lumento's `specs/transcoder/SPEC.md`, "Licensing"). fpx-convert is
built in its own repo, so its release build produces that record, in the layout lumento's
`scripts/package-third-party.sh` reads, and publishes it where lumento can fetch it with git.

## Output layout

One release produces, per architecture (`x86-64`, `arm64` — lumento's names):

```
fpx-convert/<arch>/fpx-convert
third-party/fpx-convert-<arch>/entries.tsv
third-party/fpx-convert-<arch>/licenses/<one file per licence text>
third-party/fpx-convert-<arch>/source/<tarballs>
BUILD_INFO.txt
```

`BUILD_INFO.txt` (one, for both architectures) names the fpx-convert version and commit, the exact
rustc, cargo, zig and cargo-zigbuild versions, each binary's SHA-256, and the link evidence below.
Nothing in the record carries a date: building the same commit twice gives the same files.

## `entries.tsv`

One line per component, six tab-separated fields, no header:

| # | Field | Example |
|---|---|---|
| 1 | component | `png` |
| 2 | version, with how it ships | `0.18.1 (statically linked)` |
| 3 | licence, SPDX expression | `MIT OR Apache-2.0` |
| 4 | shipped-in: the architecture | `arm64` |
| 5 | SHA-256 of the source | `<64 hex>` |
| 6 | source | `crate:png 0.18.1` |

Field 6 takes one of three forms: a bare file name (that file is in `source/`), `crate:<name>
<version>` (a crates.io crate; field 5 is its `Cargo.lock` checksum, the SHA-256 of the `.crate`
file), or `url:<exact upstream tarball URL>` (field 5 is that file's SHA-256).

## Which components get a line

Everything the final link of that architecture's binary is given, nothing else — including an
input that ends up contributing 0 bytes. One rule for every input means anyone can check the
record against the link command without a judgement call; `BUILD_INFO.txt` shows what each input
actually contributed (9Reg, 2026-10-15).

1. **fpx-convert itself.** Licence `AGPL-3.0-or-later` (also the `license` field in `Cargo.toml`).
   Source: a `git archive` of the exact commit built, `fpx-convert-<version>-<shortsha>.tar.gz`
   in `source/`; field 5 is that file's SHA-256. `<shortsha>` is always 12 hex digits, so the name
   never changes as the repo grows.
2. **Every crate in `cargo tree -e normal,no-proc-macro --target <triple>`** — build, dev and
   proc-macro dependencies run on the build machine and do not ship. Licence: the crate's
   `Cargo.toml` `license`, with the legacy crates.io `/` separator written as ` OR `, which is what
   crates.io defines it to mean. Source: `crate:<name> <version>`. Crate sources are not copied
   into `source/`; lumento fetches them from crates.io by checksum.
3. **Rust's standard library as linked**: every sysroot crate the linker is given.
   - Crates vendored from crates.io (`hashbrown`, `gimli`, `libc`, …) get crate lines exactly as in
     2, with versions and checksums from the toolchain's own `library/Cargo.lock` (rust-src). The
     vendored copy's checksum must match the lockfile's.
   - The in-tree crates (`std`, `core`, `alloc`, `panic_unwind`, `unwind`, `std_detect`, the
     `rustc-std-workspace-*` shims) share one line, "Rust standard library", versioned by rustc.
     An in-tree crate whose licence differs (`compiler_builtins`) gets its own line. Source:
     `url:` the `rustc-<version>-src.tar.xz` of the pinned rustc.
4. **The C library and runtime the final link used**, proved from the build (next section), with
   `url:` the exact upstream tarball each was compiled from. With the pinned toolchain these are
   zig's bundled musl 1.2.5 (`libc.a` and the `crt1.o`/`crti.o`/`crtn.o` start-up objects),
   zig's bundled LLVM libunwind 18.1.6, and zig's own compiler-rt (`libcompiler_rt.a`, compiled
   from zig 0.13.0's `lib/compiler_rt`; source `url:` the `zig-0.13.0.tar.xz` source tarball).

A component that cannot be pinned to an exact version and source stops the build.

## Proving what the final link used

`cargo zigbuild` hands the final link to `zig cc`, which can substitute its own C runtime for the
one rustc asks for — the 2026-10-15 investigation found it does: rustc names Rust's self-contained
musl (`rcrt1.o`, `libc.a`), and zig links its own musl, libunwind and compiler-rt instead. So the
record is derived from the link, not assumed:

1. The build runs with zig's `ZIG_VERBOSE_LINK=1`, which prints the exact `ld.lld` command, and
   rustc's `-C save-temps`, which keeps the objects that command reads. Neither changes the binary
   (checked: byte-identical with and without them).
2. The `ld.lld` command is replayed with `-Map=<file>`. The replay's output must be byte-identical
   to the shipped binary, or the build fails — that is what ties the map to the real link.
3. Every input the linker is given must be one the record covers: a crate from 2, a sysroot
   crate from 3, or zig's `libc.a`, `crt*.o`, `libunwind.a` or `libcompiler_rt.a`. Anything else
   — Rust's self-contained objects, an unknown archive — fails the build until the record is
   extended to cover it.
   zig's compiler-rt is recorded, but must also contribute 0 bytes, or the build fails: with the
   pinned toolchain Rust's `compiler_builtins` already defines every symbol, and the day that
   changes someone should look at which of compiler-rt's functions (ported from LLVM or musl)
   came in, rather than the record absorbing it silently.
4. Every crate rlib the linker is given must be in the crate list (2 or 3).
5. The musl version is read from the linked `libc.a` itself (`__libc_version` in `version.o`) and
   must equal the version the record names.
6. The rustc and zig versions must equal the pins; the upstream URLs and SHA-256s in the script are
   tied to those versions.

`BUILD_INFO.txt` carries the result: per architecture, each linker input and the bytes it
contributes.

## Licence texts

`licenses/` holds:

- each crate's own licence files from its source (`LICEN*`, `COPYING*`, `COPYRIGHT*`, `NOTICE*`,
  `UNLICENSE*` at the crate root), named `<crate>-<original file name>` — MIT's copyright line is
  the notice, so a generic MIT text is not enough;
- a crate whose declared licence has no text among its files, but whose own source carries it,
  takes it from there: `jpeg-encoder`'s IJG terms are the header of its `src/fdct.rs`
  (`jpeg-encoder-src-fdct.rs-IJG-notice`);
- one copy each of the AGPL (`fpx-convert-LICENSE`), Rust's (`COPYRIGHT-library.html` and the
  MIT and Apache-2.0 texts from the pinned toolchain's `share/doc/rust`), musl's `COPYRIGHT`,
  LLVM libunwind's `LICENSE.TXT`, and for zig's compiler-rt zig's `LICENSE` plus the LLVM
  project licence (the same `LICENSE.TXT`, which every LLVM sub-project shares).

**The build fails if any line's licence has no text:** for each SPDX identifier on a line, one of
that component's files must contain the licence's recognisable text. An `OR` needs one alternative
covered; `AND` and `WITH` need every part. An identifier the build does not recognise fails it.

## Toolchain pins

`rust-toolchain.toml` pins rustc (with both targets and `rust-src`, which the std lines are read
from). Zig is pinned in `.devcontainer/Dockerfile` and checked by the build; cargo-zigbuild, which
rewrites the link command, is pinned there too.

## Publishing: the `dist` branch

- `dist` is an orphan branch, never merged into `main`, holding only the layout above plus
  `BUILD_INFO.txt`.
- Each release is one commit on it, built from a clean checkout of `main` that matches
  `origin/main`; the publish refuses a dirty tree, another branch, or a build whose
  `BUILD_INFO.txt` names a different commit.
- The commit is tagged `fpx-convert-<version>-<shortsha>`; the branch and the tag are pushed.
- Publishing the same commit again changes nothing: the files are identical, so no new commit is
  made, and an existing tag must already point at that content.

## Decided by Claude

Flagged for review, per this directory's conventions:

- One "Rust standard library" line for the in-tree std crates rather than one line each: same
  licence, same source tarball, and none of them has its own version.
- `compiler_builtins` and libunwind are recorded with the conservative `AND` reading of their
  licence statements (`MIT AND Apache-2.0 WITH LLVM-exception AND (MIT OR Apache-2.0)` is the
  crate's own; libunwind is `Apache-2.0 WITH LLVM-exception AND (NCSA OR MIT)`, from its
  `LICENSE.TXT`: the project licence plus the legacy dual licence its older code still carries).
- zig's compiler-rt is recorded as `MIT AND Apache-2.0 WITH LLVM-exception`: zig's own MIT
  licence, plus LLVM's for the functions its sources say are ported from LLVM's compiler-rt
  (the ones ported from musl are MIT already). The same conservative reading as
  `compiler_builtins`. The installed `lib/compiler_rt` matches the `zig-0.13.0.tar.xz` source
  except 67 `*_test.zig` files, which zig's binary release ships empty and which are never
  linked.
- 12-digit short SHAs in names, so they never change length.
- The publish step is a separate script from the build: the build runs where the toolchain is (the
  devcontainer), the push where git credentials are.
