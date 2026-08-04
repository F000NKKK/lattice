# Claude Code entry point

This repository's durable workflow for Claude Code lives natively under
`.claude/` (`README.md`, `rules/`, `agents/`), ported from the
Codex-oriented `AGENTS.md`/`.codex/` documentation so both agents follow the
same role pipeline and rules without Claude Code needing to read Codex's
files directly. Read before making changes:

1. `index.md` — workspace map and dependency direction.
2. `ARCHITECTURE.md` / `ARCHITECTURE.ru.md` — relevant roadmap sections.
3. `.claude/README.md`, `.claude/rules/*.md`, and the matching role profile
   in `.claude/agents/` (`researcher.md`, `architect.md`, `implementer.md`,
   `reviewer.md`).
4. The active work item in the YouTrack `NL` project (Net Lattice) via the
   `mcp__youtrack__*` tools: find the relevant Epic (roadmap stage), its
   User Story/Task children, and any linked ADR Articles under `NL-A-1`
   before editing anything. This replaces the former `.ai/<task-name>/`
   file-based workspace, which is retired — see `@.claude/rules/youtrack.md`.

## Role pipeline

Run bounded tasks through the four role subagents defined in
`.claude/agents/`:

```text
researcher   → .claude/agents/researcher.md   (read-only: maps code/tests/docs)
architect    → .claude/agents/architect.md    (read-only: design + ADR drafts)
implementer  → .claude/agents/implementer.md  (edits: one YouTrack Task)
reviewer     → .claude/agents/reviewer.md     (read-only: independent check)
```

Dispatch each bounded task through the `Agent` tool with the matching
`subagent_type` in this order: researcher → architect → implementer →
reviewer. You (the primary agent) reconcile every handoff with the active
YouTrack Task and record the result as a comment on it (`add_issue_comment`);
advance its `Stage` field to `Done` only after reviewer findings and
verification evidence are resolved. For small, tightly-scoped work it's fine
to fold a role into your own turn instead of spawning a subagent, but still
post the evidence comment as if that role had run.

These rules apply to you directly too, not just inside a subagent. Ported
into `.claude/rules/` (native Claude Code auto-loaded imports below) so they
live as real files, not inlined prose:

@.claude/rules/youtrack.md
@.claude/rules/ci.md
@.claude/rules/research.md
@.claude/rules/files.md
@.claude/rules/git.md
@.claude/rules/versioning.md

The destructive-command restrictions in `git.md` are additionally enforced
mechanically via `permissions.deny` in `.claude/settings.json` — not just
requested in text.

## Verification

Prefer standard Rust workflow: `cargo check`, `cargo test`, `cargo fmt --
--check`, `cargo clippy` as applicable to the crates touched. Report which
commands were run and which were skipped (e.g. platform-specific tests) in
the handoff/audit entry.
