# Template: create a User Story (bounded track/slice)

```json
{
  "project": "NL",
  "summary": "<Track/slice name — e.g. \"Track A — documentation polish\">",
  "description": "<Scope of this slice, its non-goals, and how it fits the parent Epic's objective.>",
  "customFields": {
    "Type": "User Story",
    "Stage": "Backlog",
    "Role": "Researcher",
    "Sprint": "<X.Y>"
  },
  "parentIssue": "NL-<epic id>"
}
```

Break the Story into `Task` children covering the applicable model,
platform, facade, backend, test, CI, documentation, and packaging work — see
`task.md`. Every Task must roll up to a Story; do not leave bare Tasks
parented directly on an Epic except for genuinely small stages.
