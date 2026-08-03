# Git rules

- Inspect `git status --short` and `git diff --check` before and after each
  bounded task.
- Preserve unrelated user changes; never use `git reset --hard`, `git clean`,
  or broad checkout commands.
- Do not amend, rebase, force-push, or create commits unless the user asks for
  a specific Git operation.
- The GitHub repository is linked to YouTrack's VCS integration: when the user
  does ask for a commit, reference the relevant `NL-*` issue ID plainly in the
  message (e.g. a `NL-13:` prefix or `NL-13` mentioned in the body) so
  YouTrack auto-links the commit to that issue's activity feed. Do not use
  YouTrack command keywords (e.g. `fixes NL-13`, `closes NL-13`) that could
  auto-transition the issue's `Stage` — Stage transitions in this workflow are
  role-gated (reviewer confirmation required before `Done`, see
  `.codex/rules/youtrack.md`) and must stay explicit tool calls, not a side
  effect of a commit message.
- Keep patches narrow and reviewable. Separate model/API, backend, tests, and
  documentation changes when practical.
- `.codex/`, root `AGENTS.md`, and `index.md` may be intentionally ignored
  local agent context; inspect them but never force-add them. Do not change
  ignore policy unless the user explicitly asks.
- Before handoff, report changed tracked files, verification commands, and any
  privileged tests not run locally.
