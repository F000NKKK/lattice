# Net Lattice Claude Code configuration

This directory is the self-contained, native home of the repository workflow
for Claude Code — role pipeline, rules, and task-workspace templates. It was
originally ported from the Codex-oriented `.codex/` directory so both agents
follow the same workflow, but everything Claude Code needs to operate lives
here; nothing under this directory reads `.codex/` at runtime. Task-specific
plans, evidence, and decisions still live under `.ai/<task-name>/`; this
directory only holds durable, reusable configuration.

## Load order

1. `CLAUDE.md` at the repository root loads automatically at session start.
   It `@`-imports every file in `rules/` (so all rules apply to the primary
   agent unconditionally) and points at the active `.ai/<task-name>/`
   workspace.
2. Identify the active `.ai/<task-name>/` directory and read its `plan.md`,
   `AUDIT.md`, and relevant ADRs before editing anything.
3. Dispatch bounded work through the role subagents in `agents/` via the
   `Agent` tool, in order: `researcher` → `architect` → `implementer` →
   `reviewer`. Each subagent `@`-imports only the rule files relevant to its
   role (see `agents/*.md` frontmatter and body), not the full set.
4. The primary agent reconciles every subagent handoff with `plan.md` and
   records the result in the active task's `AUDIT.md`.

Claude Code has no glob/conditional rule loading (unlike some other tools):
`@`-imports are unconditional. Per-role scoping in `agents/*.md` is the only
native mechanism available to limit which rules a given piece of work loads.

## Contents

- `settings.json` — permission allowlist for common read-only/build commands
  and a denylist that mechanically blocks destructive Git operations
  (`git reset --hard`, `git clean`, force-push, amend, rebase, forced add) —
  enforcement, not just prose in `rules/git.md`.
- `rules/` — reusable audit, file, Git, research, and CI constraints, using
  Claude Code tool names directly (`Edit`/`Write` for file edits,
  `Grep`/`Glob`/`Read` for search and inspection).
- `agents/` — role subagents for research, design, implementation, and
  review, as native Claude Code subagent definitions (YAML frontmatter with
  `name`, `description`, `tools`).
- `templates/` — starting structures for a new task plan, audit log, and ADR
  (`plan.md`, `AUDIT.md`, `ADR.md`).

## Relationship to `.codex/`

`.codex/` is the equivalent workflow for Codex sessions. The two directories
are independent at runtime — Claude Code reads only `.claude/` and never
`.codex/` — but they are kept in sync by convention: when a rule or role
changes in one, mirror the change into the other so both agents follow the
same policy.

## New task workspace

Create `.ai/<task-name>/` from `.claude/templates/`. The directory must
contain:

```text
.ai/<task-name>/
├── plan.md
├── AUDIT.md
└── adr/
    └── ADR-NNNN-short-title.md
```

The plan is the authoritative TODO; `AUDIT.md` records what was inspected,
changed, and verified; ADRs (start from `templates/ADR.md`) record decisions,
alternatives, and consequences.

## Handoff contract

Every role appends an audit entry containing its role, plan checkbox,
files/symbols inspected, output, commands, unresolved risks, and next role.
The reviewer must not reuse the implementer's claim as evidence: it inspects
the diff and verification results independently.
