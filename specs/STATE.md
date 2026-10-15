# fpx-convert — state

Always current, ≤ 60 lines, every line bracketed with its command or marked inferred. Rules in
`../claude/CLAUDE.md` § "State". Created 2026-10-02.

## Where `main` is

- `main` at `783d49d` — PR #16 (a licence-record line for every linker input) merged
  [`git log --oneline -1 origin/main`, 2026-10-15].
- Crate version `0.1.0`, licence `AGPL-3.0-or-later` [`grep ^version\|^license Cargo.toml`].

## Released

- `dist` at `426189f`, tag `fpx-convert-0.1.0-783d49d609e9`: 35 entries per arch, zig
  compiler-rt included; binary sha256 x86-64 `57574ac2…`, arm64 `22e61e72…`
  [`git ls-remote origin dist`, `packaging/BUILD_INFO.txt`, 2026-10-15].
- Superseded: tag `fpx-convert-0.1.0-2a89b86c8a30` (dist `75191bb`), same binaries, 34 entries
  (no compiler-rt line). Kept, not deleted.

## Next

- Lumento picks up the record from the tag above (message handed to 9Reg 2026-10-15).

## Waiting on 9Reg

- Merge `chore/state-0.1.0-record-v2` (this file); pass the release message to Lumento.
