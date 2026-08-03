# Template: create a Task (one bounded unit of work)

```json
{
  "project": "NL",
  "summary": "<Imperative, bounded — e.g. \"Fix examples hardcoding a real interface index\">",
  "description": "<What must change: files, public API impact, platform assumptions, tests, cleanup behavior, and what counts as completion evidence.>",
  "customFields": {
    "Type": "Task",
    "Stage": "Backlog",
    "Role": "Implementer",
    "Platform": ["Cross-platform"],
    "Sprint": "<X.Y>"
  },
  "parentIssue": "NL-<story id>"
}
```

`Role` should reflect who currently owns the Task in the researcher →
architect → implementer → reviewer pipeline; update it as the Task moves
through the pipeline. `Platform` should list only the platforms the Task
actually touches (`Linux`, `Windows`, `Darwin`) or `Cross-platform` for
model/facade-only work.

## Recording progress

Post evidence as a comment on the Task via `mcp__youtrack__add_issue_comment`
(see `@.claude/rules/youtrack.md` for the required comment structure), and
advance `Stage` (`Backlog` → `Develop` → `Review` → `Test` → `Staging` →
`Done`) as work progresses. Do not set `Done` without an independent
reviewer comment confirming no unresolved defects.

## Filing a Bug found during implementation/review

```json
{
  "project": "NL",
  "summary": "<Concrete defect summary>",
  "description": "<Repro, expected vs actual, evidence.>",
  "customFields": {
    "Type": "Bug",
    "Stage": "Backlog",
    "Priority": "Major"
  }
}
```

Then `mcp__youtrack__link_issues` with `linkType: "relates to"` back to the
Task/Story it affects.
