# CLAUDE.md

Guidance for Claude Code when working in this repo, and a running record of how Greg and Claude work together on it.

## Project

fpx-convert converts FPX images into formats usable by modern web browsers. It's written in Rust so it can run efficiently on an Asustor NAS — Rust was a deliberate choice, not a default: Greg wants something others are more likely to pick up and use, not just the fastest path to done.

fpx-convert exists to support **Lumento** (same GitHub user, 9Reg, separate repo — written in Go). fpx-convert is a standalone tool, not a Lumento-internal module, so it should be designed to be useful on its own, not just wired to Lumento's specific needs.

### Build targets

Two build paths are required:
- **x86_64** — Asustor NAS
- **ARM** — other Asustor models / devices

Both need to work; don't design around only one.

### Spec-driven development

We write specs before we write code. Specs live in [specs/](specs/) and should describe *behavior and intent* independent of Rust, so they'd still be useful if a non-Rust implementation were ever needed.

## Git workflow

- Never commit directly to `main`. All work happens on a branch.
- Claude opens the PR and writes the PR description. Greg merges manually — Claude does not merge.

## How to work with Greg on this project

- **Ask one question at a time.** Stacking multiple questions in one message is annoying. Ask the most important one, wait for the answer, then ask the next if still needed.
- **Be a partner, not an order-taker.** Greg is relying on Claude's judgment, not just its hands. If a request seems off, say so directly and ask what the underlying goal is — don't silently comply, and don't silently build something different either.
- **Greg doesn't know Rust.** He chose it deliberately (portability, and the odds that others will use or contribute to it) — not out of prior Rust experience. Explain Rust-specific decisions, idioms, and tradeoffs rather than assuming familiarity. This is a learning project for him as much as a deliverable.
- **Don't assume domain expertise Greg hasn't claimed** — FPX format quirks, NAS deployment constraints, etc. Ask rather than guess.

## Repo layout

- `specs/` — spec-driven development specs (implementation-agnostic where practical)
- `test-media/` — local sample FPX images for manual testing. Gitignored; never committed.

## Notes

(Running log of things we learn as the project goes — add here as they come up.)

- **spec 0001 is implemented** (`src/`, branch `feature/fpx-conversion-pipeline`). Parses FlashPix via the `cfb` crate (CFBF/OLE2 container), a hand-rolled OLE property-set parser, decodes JPEG tiles via `jpeg-decoder`, and writes PNG + `eXIf` via the `png` crate. No FFI, no C toolchain needed — cross-compiles for both `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu` out of the devcontainer as-is.
- **FlashPix's exact binary layout isn't in the public spec text** (spec 0001 says as much — it points to primary references instead of restating them). We got byte-level ground truth from Kodak/DIG's own reference implementation, `libfpx` (github.com/ImageMagick/libfpx, Apache-1.0-like license) — not ported or linked, just read as documentation of the format — then confirmed every field byte-for-byte against a real sample file before writing the parser. Worth repeating that approach if the format ever needs revisiting.
- **Two things the spec doesn't mention that the real file layout requires:**
  - The actual image data lives one level down, inside a `Data Object Store NNNNNN` storage — not at the CFBF root. The parser finds it by searching for the telltale `Image Contents` stream rather than assuming a fixed path.
  - OLE property-set streams (`Image Contents`, `Image Info`, `SummaryInformation`) are stored with a leading control character (`U+0005`) in their names that doesn't show up in path-display output. Exact-match stream lookups have to tolerate that prefix.
- **`test-media/1997.12.25 XMas_Dads_D_4.fpx`** (gitignored, provided by Greg) is a second Kodak DC210 Zoom photo from the same shoot as the spec's reference sample — same 1152×864 resolution, timestamped ~3.5 minutes apart. Used to validate the parser end-to-end (visually and byte-for-byte against hand-decoded property values); not committed, so CI/other contributors need their own sample for full end-to-end testing — the test suite's synthetic CFBF fixtures (`tests/error_paths.rs`, plus unit tests in `propset.rs`/`subimage_header.rs`) cover error paths without needing one.
- Considered `little_exif` for writing the PNG `eXIf` chunk; its PNG write path (as of 0.6.23) actually writes a `zTXt` chunk regardless of the `as_zTXt_chunk` flag, not a real `eXIf` chunk. Hand-rolled a small TIFF/EXIF writer instead (`src/exif.rs`) — the `png` crate has first-class `eXIf` support via `Info::exif_metadata`, so this ended up simpler than pulling in the dependency anyway.
