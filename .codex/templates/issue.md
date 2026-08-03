# Templates: create YouTrack Epic/Story/Task/Bug via REST API

Base URL: `https://hush.youtrack.cloud`. All requests need
`Authorization: Bearer <token>` and `Content-Type: application/json`.

## Epic (one per roadmap stage)

```
POST /api/issues?fields=id,idReadable
{
  "project": {"id": "0-1"},
  "summary": "Stage <X.Y> — <one-line stage goal>",
  "description": "Roadmap/source: `index.md:<line>`, `ARCHITECTURE.md:<line>`.\n\n## Objective\n\n<Outcome and non-goals.>",
  "customFields": [
    {"name": "Type", "$type": "SingleEnumIssueCustomField", "value": {"name": "Epic"}},
    {"name": "Stage", "$type": "StateIssueCustomField", "value": {"name": "Backlog"}},
    {"name": "Sprint", "$type": "SingleEnumIssueCustomField", "value": {"name": "<X.Y>"}}
  ]
}
```

## User Story (bounded track/slice, child of an Epic)

```
POST /api/issues?fields=id,idReadable
{
  "project": {"id": "0-1"},
  "summary": "<Track/slice name>",
  "description": "<Scope and non-goals for this slice.>",
  "customFields": [
    {"name": "Type", "$type": "SingleEnumIssueCustomField", "value": {"name": "User Story"}},
    {"name": "Stage", "$type": "StateIssueCustomField", "value": {"name": "Backlog"}},
    {"name": "Role", "$type": "SingleEnumIssueCustomField", "value": {"name": "Researcher"}},
    {"name": "Sprint", "$type": "SingleEnumIssueCustomField", "value": {"name": "<X.Y>"}}
  ]
}
```

Then link it under the Epic:

```
POST /api/issues/<epicId>/links/subtask/issues?fields=id
{"id": "<storyId>"}
```

(Exact link-creation endpoint depends on the YouTrack version — verify
against `GET /api/issueLinkTypes` and the target issue's `links` field if
this shape errors; the durable invariant is "Story is a subtask of Epic",
not this specific request path.)

## Task (one bounded unit of work, child of a Story)

Same shape as Story, with `Type: Task`, `Role` set to the owning pipeline
role, and a `Platform` multi-value field (`Linux`/`Windows`/`Darwin`/
`Cross-platform`) reflecting what the Task actually touches. Link it under
its Story the same way Stories link under Epics.

## Bug

```
POST /api/issues?fields=id,idReadable
{
  "project": {"id": "0-1"},
  "summary": "<Concrete defect summary>",
  "description": "<Repro, expected vs actual, evidence.>",
  "customFields": [
    {"name": "Type", "$type": "SingleEnumIssueCustomField", "value": {"name": "Bug"}},
    {"name": "Stage", "$type": "StateIssueCustomField", "value": {"name": "Backlog"}},
    {"name": "Priority", "$type": "SingleEnumIssueCustomField", "value": {"name": "Major"}}
  ]
}
```

Then link it `relates to` the affected Task/Story via
`POST /api/issues/<bugId>/links?fields=id` with the appropriate
`issueLinkType` payload.

## Recording progress (audit trail)

```
POST /api/issues/<id>/comments?fields=id
{"text": "<role>: <files/symbols inspected, decisions, commands run and pass/fail, docs sync reviewed, remaining risks>"}
```

See `rules/youtrack.md` for the required comment structure and `Stage`
progression rules.

## ADR Article

```
POST /api/articles?fields=id,idReadable
{
  "project": {"id": "0-1"},
  "summary": "ADR-<NNNN> (<stage>): <Decision title>",
  "content": "- Status: proposed\n- Date: <YYYY-MM-DD>\n- Stage: <X.Y>\n\n## Context\n\n...\n\n## Decision\n\n...\n\n## Alternatives considered\n\n...\n\n## Consequences\n\n...\n\n## Verification\n\n...",
  "parentArticle": {"id": "<NL-A-1 internal id>"}
}
```

Reference the resulting Article's readable ID (e.g. `NL-A-7`) as explicit
text in the governing issue — there is no typed issue-to-article link.
