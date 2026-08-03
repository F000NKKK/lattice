# Git rules

- Inspect `git status --short` and `git diff --check` before and after each
  bounded task.
- Preserve unrelated user changes; never use `git reset --hard`, `git clean`,
  or broad checkout commands.
- Do not amend, rebase, force-push, or create commits unless the user asks for
  a specific Git operation.
- When the user does ask for a commit, it MUST name the `NL-*` issue ID(s) it
  relates to — e.g. a `NL-13:` prefix, or every relevant ID mentioned in the
  body when one commit spans more than one Task. This is mandatory on every
  commit, not optional polish: it is how a diff traces back to its
  authorizing Task/Story/Epic/Bug, and the GitHub repository is linked to
  YouTrack's VCS integration so a referenced ID also auto-links the commit to
  that issue's activity feed. Do not use YouTrack command keywords (e.g.
  `fixes NL-13`, `closes NL-13`) that could auto-transition the issue's
  `Stage` — Stage transitions in this workflow are role-gated (reviewer
  confirmation required before `Done`, see `.codex/rules/youtrack.md`) and
  must stay explicit tool calls, not a side effect of a commit message.
- If the change implements or follows a decision recorded in a YouTrack
  knowledge-base Article (an ADR under `NL-A-1`, e.g. `NL-A-7`), also name
  that Article ID in the commit message alongside the issue ID(s) — a diff
  that exists because of an ADR must be traceable back to it from the commit,
  not only from the issue description.
- Keep patches narrow and reviewable. Separate model/API, backend, tests, and
  documentation changes when practical.
- `.codex/`, root `AGENTS.md`, and `index.md` may be intentionally ignored
  local agent context; inspect them but never force-add them. Do not change
  ignore policy unless the user explicitly asks.
- Before handoff, report changed tracked files, verification commands, and any
  privileged tests not run locally.
