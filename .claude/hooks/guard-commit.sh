#!/bin/bash
# PreToolUse(Bash) guard for fpx-convert.
#
# Adapted 2026-09-21 from /Users/greg/Documents/dev/loadmento/.claude/hooks/guard-commit.sh,
# which carries the incident history these rules were written from: three commits landed on
# main despite the prose rule, and five more reached main via a branch whose upstream was
# refs/heads/main.
#
# Rules:
#   1. No commit on `main`.
#   2. No push that resolves to `main`.
#   3. No commit onto a branch whose upstream IS `main` — that branch looks healthy in
#      every UI while being wired to main, and a UI push lands there.
#   4. No commit while `cargo fmt --check` fails — this repo's own equivalent of the
#      loadmentod rule (there is no CI here; this hook is the only thing keeping the
#      workspace fmt-clean).
#
# Reads the PreToolUse payload on stdin; denies by emitting permissionDecision.
# Exits 0 in every path — a crash here must never block unrelated work.
set -uo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"   # this repo, wherever it is cloned

deny() {
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":%s}}\n' \
    "$(printf '%s' "$1" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))')"
  exit 0
}

input=$(cat)
cmd=$(printf '%s' "$input" | python3 -c 'import json,sys
try: print(json.load(sys.stdin).get("tool_input",{}).get("command",""))
except Exception: print("")' 2>/dev/null) || exit 0
[ -n "$cmd" ] || exit 0
# The checkout Claude is actually in. ${CLAUDE_PROJECT_DIR} and this script's path stay on the
# main checkout when Claude works in a git worktree; the input's `cwd` follows Claude
# (code.claude.com/docs/en/hooks, "Worktrees are different").
cwd=$(printf '%s' "$input" | python3 -c 'import json,sys
try: print(json.load(sys.stdin).get("cwd",""))
except Exception: print("")' 2>/dev/null)
top=$( [ -n "$cwd" ] && git -C "$cwd" rev-parse --show-toplevel 2>/dev/null ) || top=$REPO

# Substring, not prefix: real commit calls are compound
# (`test ... || exit 1; git add -A && git commit ...`), so a prefix match misses
# exactly the case this guard exists for.
case "$cmd" in *"git commit"*|*"git push"*) ;; *) exit 0 ;; esac

cd "$top" 2>/dev/null || exit 0

branch=$(git branch --show-current 2>/dev/null) || exit 0
upstream=$(git config --get "branch.${branch}.merge" 2>/dev/null || true)

# --- rule 2: any push whose destination resolves to main ---
case "$cmd" in
  *"git push"*)
    # Examine only the segment that invokes the push: the commit idiom CLAUDE.md
    # mandates puts a bare "main" in every commit call.
    push_seg=$(printf '%s' "$cmd" | tr ';\n' '\n\n' | sed 's/&&/\n/g; s/||/\n/g' \
               | grep -E 'git[[:space:]]+push' || true)
    if printf '%s' "$push_seg" | grep -qE '(^|[[:space:]:])main([[:space:]]|$)'; then
      deny "REFUSING: this push names main as its destination.
Open a PR instead:  git push -u origin HEAD
If you are only WRITING ABOUT a push — docs, a commit message, a spec — this guard
matches your tool call's own text. Build the literal from parts in a heredoc, write
it to a file, and commit with -F."
    fi
    # 'push -u origin HEAD' is the REPAIR for a branch wired to main — never block it.
    case "$cmd" in *"-u origin HEAD"*|*"--set-upstream origin HEAD"*) exit 0 ;; esac
    if [ "$upstream" = "refs/heads/main" ] && [ "$branch" != "main" ]; then
      deny "REFUSING: '$branch' has its upstream set to main, so this push would land ON main:
  git branch --unset-upstream && git push -u origin HEAD"
    fi
    exit 0 ;;
esac

if [ "$branch" = "main" ]; then
  deny "REFUSING: this commit would land on main. Branch first:
  git switch -c <name> origin/main && git push -u origin HEAD
(CLAUDE.md § Mechanics. On the sibling project, three commits landed on main despite the prose rule.)"
fi

# --- rule 3: committing onto a branch that is wired to main ---
if [ "$upstream" = "refs/heads/main" ] && [ "$branch" != "main" ]; then
  deny "REFUSING: '$branch' has its upstream set to refs/heads/main, so ANY push of it —
including one from the VS Code UI, which this hook cannot see — lands on main.
Repair it before committing:
  git branch --unset-upstream && git push -u origin HEAD"
fi

# --- rule 4: cargo fmt must be clean ---
# Working tree, not the index: `git add -A && git commit` has not staged anything
# yet at PreToolUse time, so an index check would always pass.
if [ -n "$(git status --porcelain -- '*.rs' 2>/dev/null)" ]; then
  if ! cargo fmt --check >/dev/null 2>&1; then
    deny "REFUSING: modified .rs files and 'cargo fmt --check' fails. Run:
  cargo fmt
(CLAUDE.md § Mechanics. There is no CI here; this hook is the only thing keeping the workspace fmt-clean.)"
  fi
fi
exit 0
