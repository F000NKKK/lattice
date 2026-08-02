# Git rules

The destructive-command restrictions below are also enforced mechanically
via `permissions.deny` in `.claude/settings.json`, not just as prose here.

- Inspect `git status --short` and `git diff --check` before and after each
  bounded task.
- Preserve unrelated user changes; never use `git reset --hard`, `git
  clean`, or broad checkout commands.
- Do not amend, rebase, force-push, or create commits unless the user asks
  for a specific Git operation.
- Keep patches narrow and reviewable. Separate model/API, backend, tests,
  and documentation changes when practical.
- `.ai/`, `.codex/`, root `AGENTS.md`, and `index.md` may be intentionally
  ignored local agent context; inspect them but never force-add them. Do not
  change ignore policy unless the user explicitly asks.
- Before handoff, report changed tracked files, verification commands, and
  any privileged tests not run locally.
