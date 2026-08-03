# YouTrack task-tracking rules

Net Lattice tracks all roadmap work in the YouTrack project `NL` at
`https://hush.youtrack.cloud/projects/NL`. This replaces the former
file-based `.ai/<task-name>/` workflow (`plan.md`, `AUDIT.md`, `adr/`),
which is retired.

Codex has no native YouTrack MCP integration configured in `config.toml`.
Use the YouTrack REST API directly via `curl` with a permanent token
(`Authorization: Bearer <token>`), requested from the user if not already
available in the environment. Key endpoints:

- `GET  /api/issues/<id>?fields=...` — read an issue.
- `POST /api/issues?fields=...` — create an issue (`project`, `summary`,
  `description`, `customFields`, and a `parent`/`links` payload for
  hierarchy).
- `POST /api/issues/<id>/comments?fields=...` — add a comment (audit entry).
- `GET  /api/issues?query=project:+NL+...` — search issues.
- `GET  /api/admin/projects/NL/customFields` — check current custom field
  schema before creating/updating issues; treat it as the source of truth,
  not this file.
- Articles: `POST /api/articles?fields=...` (ADR records), parented under
  the "Architecture Decision Records (ADR Index)" article `NL-A-1`.

## Issue hierarchy

- `Epic` — one roadmap stage (0.16, 0.17, 0.18, ...).
- `User Story` — a bounded track or slice inside a stage, subtask of its
  Epic.
- `Task` — one bounded unit of work: model, platform, backend, test, CI,
  documentation, or packaging subtask, subtask of its Story (or its Epic for
  small stages).
- `Bug` — a defect, linked `relates to`/`fixes` to the Task/Story it
  affects.

## Custom fields

- `Type` — `Epic` | `User Story` | `Task` | `Bug`.
- `Stage` (state field, drives the Kanban board) — `Backlog` → `Develop` →
  `Review` → `Test` → `Staging` → `Done`.
- `Priority` — `Show-stopper` | `Critical` | `Major` | `Normal` | `Minor`.
- `Role` — `Researcher` | `Architect` | `Implementer` | `Reviewer`, mirroring
  the pipeline in `.codex/agents/`.
- `Platform` (multi-value) — `Linux` | `Windows` | `Darwin` |
  `Cross-platform`.
- `Sprint` — roadmap stage label (`0.16`, `0.17`, `0.18`, ...), separate
  from the `Stage` workflow-state field; set it to the stage the issue
  belongs to.

## Audit trail — issue comments replace `AUDIT.md`

Every role posts its evidence as a comment on the relevant issue, in place
of the former `AUDIT.md` entry format. A comment must state: role, files/
symbols inspected, decisions and changes made (or "no edit required" with
reason), commands run and pass/fail/not-run status, documentation sync
reviewed/updated, and remaining risks/next step.

The reviewer must not reuse the implementer's comment as evidence: inspect
the diff and re-run verification independently, then post an independent
comment. Advance `Stage` to `Done` only after a reviewer comment confirms no
unresolved defects.

## ADRs — YouTrack Articles replace `adr/*.md`

Architectural or public-API decisions, cross-crate boundary changes,
compatibility-policy changes, or reversals of an accepted decision require
an Article under `NL-A-1`, using the same Context / Decision / Alternatives
considered / Consequences / Verification structure as the retired
`.codex/templates/ADR.md`. Reference the Article ID (e.g. `NL-A-7`) as
explicit text in the governing issue's description or a comment — there is
no typed issue-to-article link over this surface.

## Decomposing a stage

- Start from one Epic and decompose it into the applicable model, platform,
  facade, backend, test, CI, documentation, and packaging Tasks.
- Every Task must name its files, public API impact, platform assumptions,
  tests, cleanup behavior, and completion evidence in its description.
- Audit current code before proposing new types or traits; reuse established
  domain types, provider contracts, capabilities, and execution paths.
- Advance `Stage` to `Done` only after implementation, focused tests, docs,
  and verification gates all agree, and a reviewer comment confirms it.
- Record unresolved questions as a comment on the Epic or as a new `Task`
  with `Stage: Backlog`, not as untracked chat/session output.

## Migration note

`.ai/` is retired; do not create new `.ai/<task-name>/` workspaces. If you
find stray `.ai/` content, it predates this migration — check `NL-1`..`NL-5`
and the ADR Index (`NL-A-1`) before assuming information is lost.
