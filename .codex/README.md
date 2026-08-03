# Net Lattice Codex configuration

This directory contains reusable repository workflow, not the state of one
roadmap stage. Task-specific plans, evidence, and decisions live in the
YouTrack project `NL` (https://hush.youtrack.cloud/projects/NL), reached via
the YouTrack REST API — see `rules/youtrack.md`. The former file-based
`.ai/<task-name>/` workspace is retired.

## Load order

1. Read root `AGENTS.md` and `index.md`.
2. Identify the active YouTrack Epic (roadmap stage) and Task/Story via the
   REST API.
3. Read the issue's description, prior comments, and any linked ADR
   Articles under `NL-A-1`.
4. Load all five rule files in `rules/` (`ci.md`, `files.md`, `git.md`,
   `research.md`, `youtrack.md`). Codex has no glob/conditional or per-role
   rule loading in `config.toml` — unlike role scoping, there is no
   mechanism to load only "applicable" rules automatically, so read the
   full set every session and apply judgment about which constraints bind
   the current slice.
5. When acting as `researcher`, `architect`, `implementer`, or `reviewer`
   (`agents/*.md`), that file narrows which of the already-loaded rules are
   load-bearing for the role — it is a reading-priority convention, not an
   enforced restriction. Record an explicit `not applicable` comment on the
   issue when a mechanical slice does not need architecture work.
6. Let the primary agent reconcile every handoff with the active issue and
   record the result as a comment on it.

`config.toml` currently sets no `approval_policy` or `sandbox_mode`, so
nothing in `rules/git.md` (no `reset --hard`, no force-push, no
amend/rebase) is mechanically enforced for Codex the way the equivalent
Claude Code rules are via `permissions.deny` in `.claude/settings.json` —
these remain instructions the agent must follow deliberately, not a tool-
level block. Tightening this would mean setting `approval_policy`/
`sandbox_mode`, which changes how much Codex can do without asking; that is
a deliberate behavior change and should be a separate, explicit decision
rather than bundled into a rules-loading fix.

## Contents

- `config.toml` — minimal repository-local Codex discovery settings.
- `rules/` — reusable YouTrack, file, Git, research, and CI constraints.
- `agents/` — role profiles for research, design, implementation, and review.
- `templates/` — request bodies and field references for creating YouTrack
  Epics/Stories/Tasks/Bugs and ADR Articles via the REST API.

## Starting a new stage or task

Create the Epic/Story/Task hierarchy directly in YouTrack via the REST API,
using `templates/issue.md` as the request-body reference. Do not create a
new `.ai/<task-name>/` directory — that workflow is retired.

## Handoff contract

Every role posts an issue comment containing its role, the Task/Story it
worked, files/symbols inspected, output, commands, unresolved risks, and
next role. The reviewer must not reuse the implementer's claim as evidence:
it inspects the diff and verification results independently.
