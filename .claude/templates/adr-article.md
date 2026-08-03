# Template: create an ADR Article

Use `mcp__youtrack__create_article` with `parentArticle: "NL-A-1"` (the ADR
Index) for any public-API decision, cross-crate boundary change,
compatibility-policy change, or reversal of an accepted decision.

```json
{
  "project": "NL",
  "summary": "ADR-<NNNN> (<stage>): <Decision title>",
  "parentArticle": "NL-A-1",
  "content": "- Status: proposed\n- Date: <YYYY-MM-DD>\n- Stage: <X.Y>\n\n## Context\n\n<Problem, existing contracts, constraints, decision drivers.>\n\n## Decision\n\n<The chosen contract, precisely.>\n\n## Alternatives considered\n\n- <Alternative and why it was not selected.>\n\n## Consequences\n\n<Compatibility, platform, testing, documentation, and migration effects.>\n\n## Verification\n\n<Evidence required before changing the status to accepted.>"
}
```

Reference the resulting Article ID (e.g. `NL-A-7`) as explicit text from the
governing Task/Story's description or a comment — YouTrack has no typed
issue-to-article link over this MCP surface. Move `Status` from `proposed`
to `accepted` (via `mcp__youtrack__update_article`) only once the
Verification section's evidence is actually satisfied, mirroring how
`.claude/rules/youtrack.md` gates `Stage: Done`.
