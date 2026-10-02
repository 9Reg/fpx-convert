# CLAUDE.md

**Shared rules live in the `claude` repo, a sibling of this one.** `~/.claude/CLAUDE.md` links to
`../claude/CLAUDE.md`, made by `../claude/install.sh` (once per machine; dev containers run it
when they are created). **If the word SHARED-RULES-LOADED is not in your context above this
file, the shared rules did not load: tell 9Reg before doing anything else.** The fix is to clone
`claude` beside this repo and run `../claude/install.sh`.

This repo's state is in `STATE.md` (under `specs/` where the repo has one). Handoffs live in
Claude's auto-memory, and no PR while one exists (rules in `../claude/CLAUDE.md`).

Guidance for Claude Code when working in this repo, and a running record of how 9Reg and Claude
work together on it.

The portable rules — delegation, progressive disclosure, the Fable budget, the question gate,
extracted-vs-inferred — live in `../claude/CLAUDE.md` (loaded through `~/.claude/CLAUDE.md`) and are not repeated here.

## Project

fpx-convert converts FPX images into formats usable by modern web browsers. It's written in Rust so it can run efficiently on an Asustor NAS — Rust was a deliberate choice, not a default: 9Reg wants something others are more likely to pick up and use, not just the fastest path to done.

fpx-convert exists to support **Lumento** (same GitHub user, 9Reg, separate repo — written in Go). fpx-convert is a standalone tool, not a Lumento-internal module, so it should be designed to be useful on its own, not just wired to Lumento's specific needs.

### Build targets

Two build paths are required:
- **x86_64** — Asustor NAS
- **ARM** — other Asustor models / devices

Both need to work; don't design around only one.

### Spec-driven development

We write specs before we write code. Specs live in [specs/](specs/) and should describe *behavior and intent* independent of Rust, so they'd still be useful if a non-Rust implementation were ever needed.

**Spec first, then build — every time, no unspec'd features.** Any new or changed behavior (a new CLI flag, a new output format, a new fallback path, anything a caller could observe) gets the relevant spec doc updated *before* the implementation, not after and not skipped. If a request would add behavior with no spec coverage, update the spec as part of that same piece of work rather than letting code and spec drift apart.

## Who 9Reg is

The common section lives in `../claude/CLAUDE.md`. Bullets specific to this repo:

- **[fpx-convert] 9Reg doesn't know Rust.** He chose it deliberately (portability, and the odds
  that others will use or contribute to it) — not out of prior Rust experience. Explain
  Rust-specific decisions, idioms, and tradeoffs rather than assuming familiarity. This is a
  learning project for him as much as a deliverable.
- **[fpx-convert] Don't assume domain expertise 9Reg hasn't claimed** — FPX format quirks, NAS
  deployment constraints, etc. Ask rather than guess.

## The working method

### Design, review, plan, then build — in that order, never collapsed

Four stages, and a stage is not skipped because the work looks small. A design that has passed
review is **not** cleared to build — it still owes an implementation plan. Offering to build
straight from a reviewed design is the error this rule names.

Where the stages live: the design and the plan are `specs/` files; the review is stored verbatim
beside the design with its Disposition block; the build is the branch. `[check: a build commit's
entry cites a design file, a `*-REVIEW.md` beside it, and a plan]`

### What needs a second context before it commits

**Decided by: Claude**, per the global rule that each project names its own triggers — flag if
any of these are wrong for how this repo actually works:

- Any claim about FPX's binary layout that rests on inference rather than a byte-level check
  against a real sample file or a reference implementation (see the `libfpx` note in Notes below
  — this is exactly the kind of claim that has bitten this project before).
- A change to the release build/linking configuration (target triple, static-linking flags) —
  the musl-vs-gnu and 32-bit-cross-linker incidents in Notes below are both examples of this
  going wrong silently (binaries that only ran because the build host happened to support them).
- A fix design that **departs** from what a review specified. A fix built *as specified* is code
  under an approved design — no review.

### The cap — one review per PR

One adversarial review per PR, and only when a trigger above fires. A second review of the same
artifact only if the first returned a CRITICAL or NO. None on a prose-only branch — wording, a
spec that documents already-shipped code, an index regeneration.

**No waiver base rates exist here yet** (unlike loadmento, which has measured them across dozens
of reviews) — default to reviewing rather than waiving until this project has enough review
history to measure its own rates.

Reviewer: **a fresh Opus context**, commissioned adversarially — *"re-derive every claim from the
primary source; do not take the prose on trust."*

**Storing a finding is not discharging it.** Every stored review opens with a Disposition block,
one row per finding: FIXED with `file:line`, FILED with a Notes entry or backlog item, REJECTED
with the evidence that beat it, or ACK. Every finding is fixed now or becomes a tracked item
before the session ends — never left in the review only.

### Mechanics

- **Never on `main`.** The first command of every commit call is one that *fails* on `main`:
  `test "$(git branch --show-current)" != main || exit 1` — never an `echo`. All work happens on
  a branch (`git checkout -b <type>/<short-desc>`), landed via PR. `[enforced: PreToolUse hook,
  .claude/hooks/guard-commit.sh]`
- **`cargo fmt` before every Rust commit.** `[enforced: the same hook]`
- **A regression test is proven by its failure.** Revert each fix alone; the predicted symptom
  must appear.
- **Calls you made alone are marked `**Decided by: Claude`** in the entry. `[check: grep]`
- **Unresearched numbers are labelled** `ours, a placeholder, not measured`. `[check: grep]`
- **Every factual line in a handoff or summary carries its command in brackets, or the word
  "inferred".**

## Git workflow

**Claude is 9Reg's git helper, not just a commit-maker — 9Reg approves, Claude manages the mechanics.** Concretely:

- **Every new version/feature gets its own commit, with a detailed commit message** — not just a one-line summary. Explain what changed and why, the same way the rest of this repo's commit history and this file's Notes log do.
- **Claude owns keeping local git state correct and in sync**, so 9Reg never hits a broken "Sync Changes" in the IDE: before starting new work, fast-forward local `main` to `origin/main` and branch fresh off that — don't build on top of a branch whose content may already be merged. After a PR merges, don't reuse or keep building on that branch; prune it (`git branch -d`, `git remote prune origin`) and start the next piece of work from an up-to-date `main`.
  - *Why this rule exists:* on 2026-07-19, a feature branch got merged via PR on GitHub (which auto-deletes the head branch on merge) while local git still had it checked out and thought it was tracking a live remote branch. A new commit landed on top of that orphaned branch, and the local `main` was stale too, so the IDE's "Sync Changes" failed outright (`git pull` couldn't find the remote ref anymore). Fixed by fast-forwarding `main` and cherry-picking the new commit onto a fresh branch. Keeping `main` synced and branching fresh avoids this happening again.
- Once a branch is pushed and ready, 9Reg creates the PR and merges it himself — Claude does not open PRs and does not merge. Claude's job ends at "pushed, clean, ready for you to open the PR."

## Repo layout

- `specs/` — spec-driven development specs (implementation-agnostic where practical)
- `test-media/` — local sample FPX images for manual testing. Gitignored; never committed.

## Notes

(Running log of things we learn as the project goes — add here as they come up.)

- **spec 0001 is implemented** (`src/`, branch `feature/fpx-conversion-pipeline`). Parses FlashPix via the `cfb` crate (CFBF/OLE2 container), a hand-rolled OLE property-set parser, decodes JPEG tiles via `jpeg-decoder`, and writes PNG + `eXIf` via the `png` crate. No FFI, no C toolchain needed — cross-compiles for both `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl` out of the devcontainer as-is.
- **Release binaries are fully static (musl), not dynamically linked against glibc.** Flagged by the Lumento side (which already holds vendored binaries like `heif-convert` to a static-linking bar, `-static`, specifically so they don't depend on whatever libc the NAS firmware ships) — the original gnu-target builds were dynamically linked against `libc.so.6`/`ld-linux`, a real portability gap for a NAS deployment target. Switched `x86_64-unknown-linux-gnu`/`aarch64-unknown-linux-gnu` to `x86_64-unknown-linux-musl`/`aarch64-unknown-linux-musl`; all deps (`cfb`, `jpeg-decoder`, `png`, `thiserror`) are pure Rust with no C bindings, so this was a target swap, not a toolchain fight. Verified: both release binaries report `statically linked` (`file`) / `not a dynamic executable` (`ldd`), and the aarch64 musl build's PNG output is byte-identical to the prior aarch64 gnu build on the real sample file — no behavioral regression.
  - Musl cross-linking needed a cross-linker, and the obvious choice (prebuilt gcc toolchains from musl.cc, what most Rust-cross-to-musl tutorials point to) turned out to ship as 32-bit x86 binaries — they only ran here because this devcontainer host had x86 emulation available; a host without it would fail the Dockerfile build outright. Used `cargo-zigbuild` + Zig instead: Zig ships genuine native binaries per host OS/arch from ziglang.org, so the Dockerfile works the same regardless of what machine builds the image. `.devcontainer/Dockerfile` now installs Zig (version-pinned) instead of the old `gcc-{x86-64,aarch64}-linux-gnu` cross packages, and `.cargo/config.toml` (manual per-target linker mapping) was deleted — `cargo zigbuild` handles that itself.
- **FlashPix's exact binary layout isn't in the public spec text** (spec 0001 says as much — it points to primary references instead of restating them). We got byte-level ground truth from Kodak/DIG's own reference implementation, `libfpx` (github.com/ImageMagick/libfpx, Apache-1.0-like license) — not ported or linked, just read as documentation of the format — then confirmed every field byte-for-byte against a real sample file before writing the parser. Worth repeating that approach if the format ever needs revisiting.
- **Two things the spec doesn't mention that the real file layout requires:**
  - The actual image data lives one level down, inside a `Data Object Store NNNNNN` storage — not at the CFBF root. The parser finds it by searching for the telltale `Image Contents` stream rather than assuming a fixed path.
  - OLE property-set streams (`Image Contents`, `Image Info`, `SummaryInformation`) are stored with a leading control character (`U+0005`) in their names that doesn't show up in path-display output. Exact-match stream lookups have to tolerate that prefix.
- **`test-media/1997.12.25 XMas_Dads_D_4.fpx`** (gitignored, provided by 9Reg) is a second Kodak DC210 Zoom photo from the same shoot as the spec's reference sample — same 1152×864 resolution, timestamped ~3.5 minutes apart. Used to validate the parser end-to-end (visually and byte-for-byte against hand-decoded property values); not committed, so CI/other contributors need their own sample for full end-to-end testing — the test suite's synthetic CFBF fixtures (`tests/error_paths.rs`, plus unit tests in `propset.rs`/`subimage_header.rs`) cover error paths without needing one.
- Considered `little_exif` for writing the PNG `eXIf` chunk; its PNG write path (as of 0.6.23) actually writes a `zTXt` chunk regardless of the `as_zTXt_chunk` flag, not a real `eXIf` chunk. Hand-rolled a small TIFF/EXIF writer instead (`src/exif.rs`) — the `png` crate has first-class `eXIf` support via `Info::exif_metadata`, so this ended up simpler than pulling in the dependency anyway.
- **Added JPEG as an opt-in output format** (`--format png|jpeg`, default `png`) alongside spec 0001's original PNG-only output — spec updated first (per the spec-first rule above), then `src/jpeg_writer.rs` added. Uses the `jpeg-encoder` crate (pure Rust, same no-C-toolchain constraint that drove the PNG encoder choice); its `add_exif_metadata` takes the exact same raw-TIFF payload `src/exif.rs` already builds for the PNG `eXIf` chunk and wraps it in the JPEG APP1 `Exif\0\0` header itself, so no format-specific EXIF-building code was needed. JPEG quality is a fixed internal constant (90), not caller-configurable — wasn't asked for and adding a knob nobody requested would be scope creep.
- **Release output moved from `dist/` to `packaging/`, with per-arch subdirectories renamed from Rust target triples to short names** (`packaging/x86_64/`, `packaging/arm64/`, not `packaging/x86_64-unknown-linux-musl/`) — this is the default delivery path for shipping binaries, kept separate from `target/` (raw `cargo build` output, also gitignored, not meant to be shipped from). `scripts/build-release.sh` and `.gitignore` updated together so the two never drift apart.

## Inter-session memory

State lives in this repo — `specs/STATE.md`, which travels with the clone. The session handoff
lives in Claude's auto-memory for this repo, and **no PR while one exists**; the rules are in
`../claude/CLAUDE.md`. Auto-memory otherwise holds only **facts about the machine it sits on** —
toolchain notes — because it is keyed by this machine's path and does not follow a clone.
9Reg's preferences in his words go in the `claude` repo. Never lessons about how to reason; those
measured zero effect on the sibling projects that tried them.
