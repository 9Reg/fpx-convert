# 0001 — FPX → Web Image Conversion Pipeline

**Status:** Draft — open for review
**Scope:** parse → convert → CLI output. No HTTP serving, no FFI (see [Non-goals](#non-goals-and-why)).

## Summary

fpx-convert is a command-line tool. Given one FlashPix (`.fpx`) image, it:

1. Finds the best (highest-resolution) image data actually stored in the file,
2. Decodes it, and
3. Writes it out as a PNG (lossless, the default) or JPEG (lossy, opt-in via `--format`) a modern web browser can display directly.

It converts one file at a time and exits. Nothing about it is Lumento-specific — Lumento (a separate Go project, same author) is expected to call it as a subprocess, but any other program in any language could do the same.

## Background: what's actually inside a `.fpx` file

This section exists because the FlashPix format is obscure and mostly undocumented outside of 1990s-era PDFs — everything below was either confirmed against the official spec or verified directly against `test-media/1997.12.25 XMas_Dads_GC_1.fpx`, our one real sample so far (a photo shot on a **Kodak DC210 Zoom**, Dec 25 1997 — both facts recovered straight from the file's own metadata).

- **Container:** a `.fpx` file is a Compound File Binary Format (CFBF, aka OLE2/"structured storage") container — the same container format old `.doc`/`.xls` files used. It's a filesystem-in-a-file: "storages" are folders, "streams" are files.
- **Resolution pyramid:** FlashPix can store an image at multiple resolutions, each half the width/height of the next, so a viewer can grab a small version without decoding the full-size one. Each stored resolution lives in its own `Resolution NNNN` storage.
  - **Important gotcha, confirmed against our sample:** the `NNNN` number is *not* a simple count of how many resolutions are present — it reflects a position in FlashPix's theoretical full pyramid. Our sample has exactly **one** resolution stored, yet its storage is named `Resolution 0005`. **Never infer "which is highest-resolution" from the storage number.** Read each storage's actual width/height and compare those.
- **Tiling:** each resolution level is cut into tiles (64×64 pixels by default — but the real tile size is written in the header, so read it, don't hardcode it) stored in a `Subimage NNNN Data` stream, indexed by a `Subimage NNNN Header` stream that records each tile's size/location.
- **Tile compression:** tiles are independently JPEG-compressed, but they don't each carry their own quantization/Huffman tables — those are stored **once**, shared, in the `Image Contents` property-set stream (confirmed in our sample: a property whose value is literally a JPEG byte-stream fragment starting `FF D8 FF DB...`, i.e. an SOI marker followed by quantization/Huffman tables and nothing else). A tile decoder has to combine each tile's compressed bytes with these shared tables.
- **Why this bounds what "lossless" can mean:** the pixels in the file already went through the camera's own JPEG encoder in 1997. There is no unlossy original hiding underneath. "Lossless conversion" in this spec means *fpx-convert introduces no additional generation of lossy recompression* — not that the output recovers detail the camera already discarded. Worth knowing going in so the output isn't judged against the wrong expectation.

Primary references (for whoever implements the byte-level parsing — this spec deliberately doesn't restate the full binary layout):
- [Original 1996 Kodak FlashPix spec](http://graphcomp.com/info/specs/livepicture/fpx.pdf)
- [W3C-hosted official FPX spec](https://www.w3.org/Graphics/FlashPix/FPX-spec.pdf)
- [FlashPix architecture white paper](https://www.w3.org/Graphics/FlashPix/FPwhite.pdf)
- `libfpx` — an open-source reference implementation released by Kodak/the Digital Imaging Group in 1999 (Apache-1.0-like license). Useful to cross-check parsing logic against; not being ported or copied, just consulted.

## Scope

### In scope (v1)

- Reading a single `.fpx` file and selecting its best available resolution
- Decoding that resolution's tiles into a full pixel image
- Encoding that image as a browser-displayable PNG or JPEG, caller's choice (see [Convert stage](#convert-stage))
- A CLI with both a file-path mode and a stdin/stdout streaming mode

### Non-goals (and why)

- **No HTTP serving / long-running service.** Decided in discussion for this spec: nothing about this project's actual scale (a personal photo archive, on a NAS) needs a persistent process, a port, or request concurrency. If a real need for that shows up later, it's a deliberate addition then, not a default now.
- **No FFI / library bindings for direct Go interop.** Also decided in discussion: cgo carries real cross-language memory-ownership risk and complicates cross-compiling for *two* architectures at once. A subprocess CLI gets Lumento everything it needs (`os/exec`, with either file paths or piped stdin/stdout) with none of that risk, and stays usable by anything else that can run a binary.
- **No batch/directory mode.** One file in, one file out, per invocation. A caller that wants to convert a folder loops over it itself.
- **No writing back to FPX.** One-directional only.
- **No support for non-JPEG tile compression** (the format also allows uncompressed, single-color, and LZH-compressed tiles). We have no sample exercising these. v1 should detect and clearly error on them rather than guess — see [Error handling](#error-handling).
- **No size-based lossy fallback.** Originally speced as "lossless unless it blows the size," then dropped after discussion: at the resolutions FlashPix cameras actually shot (~1-2 megapixels), a lossless PNG realistically lands around 1-5MB even in a worst case — trivial for a browser to load or a NAS to store. The cutoff was defensive engineering against a problem the format's own physical limits make very unlikely to occur. Revisit only if a real oversized file actually turns up.

## Parse stage

### Input

One `.fpx` file: either a path argument or bytes piped via stdin.

### Steps

1. Verify the CFBF signature (`D0 CF 11 E0 A1 B1 1A E1`) and open it as a compound file. Reject anything else with a clear error.
2. Enumerate every `Resolution NNNN` storage. For each, read its actual pixel width/height from its `Subimage 0000 Header`. Select the one with the largest width × height as "best available" — per the gotcha above, never assume based on the `NNNN` suffix.
3. Read the shared JPEG quantization/Huffman tables from the `Image Contents` property set.
4. Walk the selected resolution's tile table (in `Subimage 0000 Header`) and, for each tile: decode its JPEG-compressed bytes (from `Subimage 0000 Data`) using the shared tables, placing decoded pixels at the tile's position in the full image. Tiles at the right/bottom edge may be partial where the image dimensions aren't an exact multiple of the tile size — crop to the real image bounds.
5. Produce one full-resolution pixel buffer, ready for the convert stage.

### Metadata

Capture date and camera model, read from the `SummaryInformation`/`Image Info` property sets (e.g., our sample: `DC210 Zoom (V01.02)`, captured 1997-12-25 15:26:15), must be preserved in the output as EXIF. Not needed for a browser to render the image, but this is an archive of real memories, not throwaway images, and that context shouldn't be silently dropped just because the display path doesn't need it. Written as a PNG `eXIf` chunk.

### Error handling

- Bad/non-CFBF input → clear error naming the problem, non-zero exit. No partial/garbage output.
- Tile compression other than JPEG → clear error naming which compression type was found and that it's unsupported, non-zero exit.
- Any other structural surprise (missing expected stream, unreadable property) → clear error identifying what was expected and missing, non-zero exit.

## Convert stage

### Which resolution

Always the single "best available" resolution chosen during parsing. v1 does not produce multiple output sizes (e.g. a `srcset` of thumbnails) — that's a plausible future enhancement, not v1 scope.

### Output format

The caller selects **PNG** (lossless) or **JPEG** (lossy), defaulting to PNG when not specified — see [CLI interface](#cli-interface). No size cutoff, no automatic lossy fallback (see [Non-goals](#non-goals-and-why) for why that was dropped); the caller always gets exactly the format it asked for, or the PNG default if it didn't ask.

**Why PNG, not WebP/AVIF:** PNG has a mature, dependency-free (pure-Rust) encoder, which matters a lot given the two-architecture (x86_64 + ARM) build requirement — a pure-Rust dependency cross-compiles with just a target added; anything wrapping a C library (like most performant WebP/AVIF encoders) needs a full C cross-toolchain configured for *each* target, which is real, avoidable pain. PNG is also supported in every browser, not just "modern" ones. WebP/AVIF are reasonable size-optimization upgrades to revisit later — not needed to ship v1.

**Why JPEG was added:** some callers would rather take the smaller, lossier file (e.g. thumbnails, bandwidth-constrained delivery) than a PNG of a photo that was already through a lossy JPEG encoder in-camera decades ago — the "lossless" PNG guarantee (see [above](#background-whats-actually-inside-a-fpx-file)) was never recovering detail beyond what the camera itself kept. JPEG output uses the same pure-Rust cross-compilation constraint as PNG: a dependency-free Rust encoder, no C toolchain per target.

Camera model and capture date are preserved as EXIF in either format: a PNG `eXIf` chunk for PNG output, a JPEG APP1 `Exif` segment for JPEG output — same underlying TIFF payload, different container.

## CLI interface

### Output format selection

An optional `--format png|jpeg` flag (default `png`) selects the output encoding, in both file-path and stdin/stdout mode. Unrecognized values are a usage error, not a silent fallback to the default.

### File-path mode

```
fpx-convert <input.fpx> <output.png>
fpx-convert --format jpeg <input.fpx> <output.jpg>
```

The caller gives the exact output path it wants (extension included). The output file's *contents* are always encoded per `--format` (PNG unless `--format jpeg` is given) — fpx-convert does not infer format from the output path's extension, so a mismatched extension (e.g. `--format jpeg out.png`) is honored as given, not corrected.

### Stdin/stdout streaming mode

```
fpx-convert --stdin --stdout
fpx-convert --stdin --stdout --format jpeg
```

Reads the `.fpx` bytes from stdin, writes the converted image bytes (PNG unless `--format jpeg` is given) to stdout.

### Exit codes

`0` on success. Non-zero on any parse or convert error, with a human-readable message on stderr (see [Error handling](#error-handling)).

## Reference sample

`test-media/1997.12.25 XMas_Dads_GC_1.fpx` — Kodak DC210 Zoom, captured 1997-12-25 15:26:15, 1152×864 pixels, exactly one resolution level stored (named `Resolution 0005`), JPEG-tiled. Facts above were confirmed directly against this file, not just secondary sources.
