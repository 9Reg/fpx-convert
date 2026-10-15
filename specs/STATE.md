# fpx-convert — state

Always current, ≤ 60 lines, every line bracketed with its command or marked inferred. Rules in
`../claude/CLAUDE.md` § "State". Created 2026-10-02.

## Where `main` is

- `main` at `50b8402` — PR #14 (CLAUDE.md cut to core) merged
  [`git log --oneline -1 origin/main`, 2026-10-15].
- Crate version `0.1.0`, licence `AGPL-3.0-or-later` on `feat/release-licence-record`
  [`grep ^version\|^license Cargo.toml`, 2026-10-15].

## In flight

- `feat/release-licence-record`: spec 0002 (release licence record, `dist` branch), toolchain
  pins, `scripts/build-release.sh` + `scripts/licence-record.py` + `scripts/publish-dist.sh`.
  Both arches build and pass every check; two builds are byte-identical
  [`./scripts/build-release.sh` twice in the devcontainer image, `shasum` of packaging/, 2026-10-15].

## Next

- After the branch merges: run `./scripts/build-release.sh` from `main` in the devcontainer, then
  `./scripts/publish-dist.sh` on the host; hand Lumento the tag, dist commit, both `entries.tsv`
  and the link evidence from `BUILD_INFO.txt` (inferred: the task's last step).

## Waiting on 9Reg

- Open and merge the PR for `feat/release-licence-record`.
