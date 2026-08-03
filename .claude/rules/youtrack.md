# YouTrack task-tracking rules

Net Lattice tracks all roadmap work in the YouTrack project `NL`
(https://hush.youtrack.cloud/projects/NL), via the `youtrack` MCP tools
(`mcp__youtrack__*`). This replaces the former file-based `.ai/<task-name>/`
workflow (`plan.md`, `AUDIT.md`, `adr/`), which has been retired.

## Issue hierarchy

- `Epic` — one roadmap stage (0.16, 0.17, 0.18, ...). Created once per stage.
- `User Story` — a bounded track or slice inside a stage (e.g. "Track A —
  documentation polish"), linked as a subtask of its Epic.
- `Task` — one bounded checkbox-equivalent unit of work: model, platform,
  backend, test, CI, documentation, or packaging subtask. Linked as a subtask
  of its Story (or directly of its Epic for small stages).
- `Bug` — a defect found during implementation or review. Link with
  `relates to` / `fixes` to the Task/Story it affects; do not silently fold
  bug fixes into an unrelated Task.

Use `mcp__youtrack__create_issue` with `parentIssue` to build this hierarchy;
use `mcp__youtrack__link_issues` for non-parent relations (`relates to`,
`blocked by`, `duplicates`).

## Custom fields

Every issue in `NL` carries:

- `Type` — `Epic` | `User Story` | `Task` | `Bug`.
- `Stage` (state field, drives the Kanban board) — `Backlog` → `Develop` →
  `Review` → `Test` → `Staging` → `Done`. This is the workflow-state field;
  advance it as work progresses. Do not leave a Task at `Done` without an
  independent reviewer pass recorded (see below).
- `Priority` — `Show-stopper` | `Critical` | `Major` | `Normal` | `Minor`.
  Set explicitly for `Bug` issues; optional for `Task`/`Story`.
- `Role` — `Researcher` | `Architect` | `Implementer` | `Reviewer`. Marks
  which role pipeline stage currently owns the issue, mirroring the
  researcher → architect → implementer → reviewer pipeline in
  `.claude/agents/`.
- `Platform` (multi-value) — `Linux` | `Windows` | `Darwin` |
  `Cross-platform`. Set on Task/Bug issues whose scope is platform-specific;
  use `Cross-platform` for model/facade-only work.

Check the live schema with `mcp__youtrack__get_issue_fields_schema` before
creating issues if uncertain about current field values — the schema is the
source of truth, not this file.

## Audit trail — issue comments replace `AUDIT.md`

Every role appends its evidence as a comment on the relevant issue via
`mcp__youtrack__add_issue_comment`, in place of the former `AUDIT.md` entry
format. A comment must state:

- role (researcher/architect/implementer/reviewer/primary);
- files/symbols inspected;
- decisions and changes made (or "no edit required" with reason);
- commands run and pass/fail/not-run status;
- documentation sync reviewed/updated;
- remaining risks and the next bounded step.

The reviewer must not reuse the implementer's comment as evidence: it
inspects the diff and re-runs verification independently, then posts its own
comment. Advance `Stage` to `Done` only after a reviewer comment is present
with no unresolved confirmed defects.

## ADRs — YouTrack Articles replace `adr/*.md`

Architectural or public-API decisions, cross-crate boundary changes,
compatibility-policy changes, or reversals of an accepted decision require an
Article in the `NL` knowledge base (via `mcp__youtrack__create_article`),
filed under the "Architecture Decision Records (ADR Index)" parent article
(`NL-A-1`). Use the same Context / Decision / Alternatives considered /
Consequences / Verification structure the retired `.claude/templates/ADR.md`
used. Link the Article from the Task/Story it governs by referencing its ID
(e.g. `NL-A-7`) in the issue description or a comment — YouTrack does not
support issue-to-article typed links via this MCP surface, so the reference
must be explicit text.

Never propose or accept a public-API decision only in chat or only in an
issue comment; if it changes a public contract, it needs an Article.

## Decomposing a stage

- Start from one Epic (a roadmap stage) and decompose it into the applicable
  model, platform, facade, backend, test, CI, documentation, and packaging
  Tasks, same as the retired `audit.md` decomposition rule.
- Every Task must name its files, public API impact, platform assumptions,
  tests, cleanup behavior, and completion evidence in its description.
- Audit current code before proposing new types or traits; reuse established
  domain types, provider contracts, capabilities, and execution paths.
- Advance `Stage` to `Done` only after implementation, focused tests, docs,
  and verification gates all agree, and a reviewer comment confirms it.
- Record unresolved questions as a comment on the Epic or as a new `Task`
  with `Stage: Backlog`, not as untracked chat output.

## Migration note

`.ai/` is retired; do not create new `.ai/<task-name>/` workspaces. If you
find stray `.ai/` content, it predates this migration — check
`NL-1`..`NL-5` and the ADR Index (`NL-A-1`) before assuming information is
lost, then flag it for cleanup rather than silently recreating file-based
tracking.
