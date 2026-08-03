# Git rules

The destructive-command restrictions below are also enforced mechanically
via `permissions.deny` in `.claude/settings.json`, not just as prose here.

- Inspect `git status --short` and `git diff --check` before and after each
  bounded task.
- Preserve unrelated user changes; never use `git reset --hard`, `git
  clean`, or broad checkout commands.
- Do not amend, rebase, or force-push unless the user asks for that specific
  Git operation.
- Auto-commit is authorized standing policy for this repository: after each
  bounded slice (a YouTrack Task, or any reviewer-accepted edit) reaches a
  clean, verified state (relevant `cargo check`/`test`/`fmt`/`clippy` gates
  pass and the evidence comment is posted per `@.claude/rules/youtrack.md`),
  create a commit for the changed tracked files without asking first. Stage
  narrowly (named paths, never `git add -A`/`.`), keep the message focused
  on the "why" per the root git guidance, and never bypass hooks
  (`--no-verify`) to force one through.
- Commit messages (auto-commits and user-requested commits alike) must NOT
  include a `Co-Authored-By` trailer or any other attribution footer. This
  overrides the default Claude Code commit template for this repository.
- Auto-commits still respect every other restriction in this file: no
  amend/rebase/force-push, no destructive staging, no committing `.codex/`,
  root `AGENTS.md`, or `index.md`.
- Keep patches narrow and reviewable. Separate model/API, backend, tests,
  and documentation changes when practical.
- `.codex/`, root `AGENTS.md`, and `index.md` may be intentionally ignored
  local agent context; inspect them but never force-add them. Do not change
  ignore policy unless the user explicitly asks.
- Before handoff, report changed tracked files, verification commands, and
  any privileged tests not run locally.
