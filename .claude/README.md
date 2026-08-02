# Net Lattice Claude Code configuration

This directory contains reusable repository workflow for Claude Code, ported
from `.codex/` so the same role pipeline and rules apply regardless of which
agent is driving. Task-specific plans, evidence, and decisions still live
under `.ai/<task-name>/`; this directory only holds durable, reusable
configuration.

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
- `rules/` — reusable audit, file, Git, research, and CI constraints, ported
  1:1 from `.codex/rules/` with Codex-specific tool names mapped to Claude
  Code equivalents (`apply_patch` → `Edit`/`Write`, `rg`/`sed`/`awk` →
  `Grep`/`Glob`/`Read`).
- `agents/` — role subagents for research, design, implementation, and
  review, ported from `.codex/agents/` as native Claude Code subagent
  definitions (YAML frontmatter with `name`, `description`, `tools`).

## Relationship to `.codex/`

`.codex/` remains the source of truth for Codex sessions. When a rule or role
changes there, mirror the change into `.claude/rules/` or `.claude/agents/`
so both agents stay consistent — `.claude/` is a port, not an independent
policy.

## New task workspace

Unchanged from `.codex/README.md`: create `.ai/<task-name>/` from
`.codex/templates/` (`plan.md`, `AUDIT.md`, `adr/README.md`,
`adr/ADR-NNNN-*.md`). The plan is the authoritative TODO; `AUDIT.md` records
what was inspected, changed, and verified; ADRs record decisions,
alternatives, and consequences.

## Handoff contract

Every role appends an audit entry containing its role, plan checkbox,
files/symbols inspected, output, commands, unresolved risks, and next role.
The reviewer must not reuse the implementer's claim as evidence: it inspects
the diff and verification results independently.
