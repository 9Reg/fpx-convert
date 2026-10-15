# fpx-convert — state

Always current, ≤ 60 lines, every line bracketed with its command or marked inferred. Rules in
`../claude/CLAUDE.md` § "State". Created 2026-10-02.

## Where `main` is

- `main` at `2a89b86` — PR #15 (spec 0002: release licence record, `dist` branch) merged
  [`git log --oneline -1 origin/main`, 2026-10-15].
- Crate version `0.1.0`, licence `AGPL-3.0-or-later` [`grep ^version\|^license Cargo.toml`].

## Released

- `dist` at `75191bbf`, tag `fpx-convert-0.1.0-2a89b86c8a30`: built by 9Reg from `main` in the
  devcontainer, published with `scripts/publish-dist.sh`; binary sha256 x86-64 `57574ac2…`,
  arm64 `22e61e72…` [`git ls-remote origin dist`, `packaging/BUILD_INFO.txt`, 2026-10-15].

## In flight

- `feat/record-every-linker-input` (stacked on `chore/state-dist-published`): spec 0002 and
  `licence-record.py` now give every linker input a line, 0-byte ones included (9Reg's choice,
  2026-10-15), which adds `zig compiler-rt` (35 lines per arch). Binaries unchanged
  [`./scripts/build-release.sh` in the devcontainer image, 2026-10-15].

## Next

- After merge: 9Reg rebuilds from `main` in the devcontainer, then `scripts/publish-dist.sh`
  makes the second `dist` commit with a new tag (the code commit changes).
- Hand Lumento the tag, dist commit, both `entries.tsv` and the link evidence.

## Waiting on 9Reg

- Merge `feat/record-every-linker-input` (it carries `chore/state-dist-published` too).
