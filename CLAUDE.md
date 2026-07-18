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
