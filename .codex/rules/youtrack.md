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
  belongs to. The user creates and manages Sprint entities directly in
  YouTrack (Board → Sprints); agents only ever set the field on an issue,
  never create a Sprint. If the target stage has no Sprint yet, ask the
  user to create it rather than inventing one. `Sprint` is what both Agile
  boards use to scope which issues are "in view" for a stage.

## When an issue goes to `Stage: Backlog`

`Backlog` is the default/starting state, not a dumping ground:

- Every newly created Epic/Story/Task/Bug starts at `Stage: Backlog` unless
  work begins immediately in the same turn (then set `Develop` directly).
- An issue moves *back* to `Backlog` only if work is explicitly paused or
  descoped from the current Sprint — not to "park" something half-done;
  half-done work stays at its current `Stage` with a comment explaining
  what's left.
- Unresolved questions/decisions get filed as their own `Task` (or a
  comment on the Epic) at `Stage: Backlog` — never left only in the session
  transcript.

## Picking the next Task to work

Search before starting new work:

```
GET /api/issues?query=project:+NL+Sprint:+{0.18}+Type:+Task+Stage:+Backlog&fields=idReadable,summary
```

Prefer, in order: (1) a `Task` already `Stage: Develop`/`Review` with an
owning `Role` matching the role about to run — finish in-flight work first;
(2) the oldest unblocked `Stage: Backlog` `Task` in the active Sprint whose
parent Story is not itself blocked; (3) file a new `Task` if the
researcher/architect pass surfaced one that doesn't exist yet. Check
`blocked by` links before starting — do not start a blocked Task.

## Searching YouTrack

Search before creating an issue (avoid duplicates) and before assuming
information is lost (check comments/history first):

- `project: NL Sprint: {0.18}` — everything in a roadmap stage.
- `project: NL Type: Task Stage: Backlog Sprint: {0.18}` — unstarted work.
- `project: NL Role: Implementer Stage: Develop` — Tasks mid-implementation.
- `project: NL Type: Bug Stage: -Done` — open bugs.
- `project: NL Platform: Windows` — Windows-specific issues.
- Free text: `project: NL neighbor mutation` — matches summary/description/
  comments.

`GET /api/issues/<id>/comments?fields=text,author(login),created` to read
an issue's full audit trail before adding a new comment. Search Articles
(`GET /api/articles?query=...`) before drafting a new ADR.

## Boards

Two Agile boards exist on `NL`, both columned by `Stage`; they show the
same underlying issues grouped differently:

- **"Net Lattice: доска Kanban"** — swimlanes by `Epic`. Stage-level
  progress: direct children of an Epic (Stories) show as cards; anything
  nested deeper (a Task under a Story) does not appear here.
- **"Net Lattice: by User Story"** — swimlanes by the nearest parent issue
  regardless of Type, so Tasks show as cards grouped under their owning
  Story. Use for day-to-day work on a Story's Task breakdown.

Both boards are UI-configured. Codex has no board-management endpoint
exercised in this workflow — flag a needed board change to the user instead
of attempting a workaround via issue links.

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
