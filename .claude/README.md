# Net Lattice Claude Code configuration

This directory is the self-contained, native home of the repository workflow
for Claude Code — role pipeline, rules, and issue-creation templates. It was
originally ported from the Codex-oriented `.codex/` directory so both agents
follow the same workflow, but everything Claude Code needs to operate lives
here; nothing under this directory reads `.codex/` at runtime. Task-specific
plans, evidence, and decisions live in the YouTrack project `NL`
(https://hush.youtrack.cloud/projects/NL), reached via the `mcp__youtrack__*`
tools — this directory only holds durable, reusable configuration.

## Load order

1. `CLAUDE.md` at the repository root loads automatically at session start.
   It `@`-imports every file in `rules/` (so all rules apply to the primary
   agent unconditionally) and points at the active YouTrack Epic/Task.
2. Identify the active Epic (roadmap stage) and Task/Story via
   `mcp__youtrack__search_issues`/`get_issue`; read its description, prior
   comments, and any linked ADR Articles under `NL-A-1` before editing
   anything.
3. Dispatch bounded work through the role subagents in `agents/` via the
   `Agent` tool, in order: `researcher` → `architect` → `implementer` →
   `reviewer`. Each subagent `@`-imports only the rule files relevant to its
   role (see `agents/*.md` frontmatter and body), not the full set.
4. The primary agent reconciles every subagent handoff with the active
   Task/Story and records the result as a YouTrack comment.

Claude Code has no glob/conditional rule loading (unlike some other tools):
`@`-imports are unconditional. Per-role scoping in `agents/*.md` is the only
native mechanism available to limit which rules a given piece of work loads.

## Contents

- `settings.json` — permission allowlist for common read-only/build commands
  and a denylist that mechanically blocks destructive Git operations
  (`git reset --hard`, `git clean`, force-push, amend, rebase, forced add) —
  enforcement, not just prose in `rules/git.md`.
- `rules/` — reusable YouTrack, file, Git, research, and CI constraints,
  using Claude Code tool names directly (`Edit`/`Write` for file edits,
  `Grep`/`Glob`/`Read` for search and inspection,
  `mcp__youtrack__*` for issue tracking).
- `agents/` — role subagents for research, design, implementation, and
  review, as native Claude Code subagent definitions (YAML frontmatter with
  `name`, `description`, `tools`).
- `templates/` — request-body references for creating YouTrack Epics
  (`epic.md`), Stories (`story.md`), Tasks/Bugs (`task.md`), and ADR
  Articles (`adr-article.md`).

## Relationship to `.codex/`

`.codex/` is the equivalent workflow for Codex sessions. The two directories
are independent at runtime — Claude Code reads only `.claude/` and never
`.codex/` — but they are kept in sync by convention: when a rule or role
changes in one, mirror the change into the other so both agents follow the
same policy.

## Starting a new stage or task

Follow `templates/epic.md` → `templates/story.md` → `templates/task.md` to
create the Epic/Story/Task hierarchy directly in the `NL` YouTrack project.
Do not create a `.ai/<task-name>/` directory — that workflow is retired; see
`@.claude/rules/youtrack.md` for the full field/hierarchy contract.

## Handoff contract

Every role posts a YouTrack comment containing its role, the Task/Story it
worked, files/symbols inspected, output, commands, unresolved risks, and
next role. The reviewer must not reuse the implementer's claim as evidence:
it inspects the diff and verification results independently.
