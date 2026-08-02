# Claude Code entry point

This repository's durable workflow is defined in `AGENTS.md` (repo root) and
`.codex/` (README, `rules/`, `agents/`, `templates/`). That documentation was
written for Codex but applies to any agent working in this repo, including
Claude Code. Read it before making changes:

1. `index.md` — workspace map and dependency direction.
2. `ARCHITECTURE.md` / `ARCHITECTURE.ru.md` — relevant roadmap sections.
3. `.codex/README.md`, `.codex/rules/*.md`, and the matching role profile in
   `.codex/agents/` (`researcher.md`, `architect.md`, `implementer.md`,
   `reviewer.md`).
4. The active task workspace at `.ai/<task-name>/` (currently `.ai/0.17/`):
   its `plan.md`, `AUDIT.md`, and `adr/` records. `.ai/` is gitignored —
   intentional local agent context, not published crate content.

## Role pipeline

Run bounded tasks through the four role subagents defined in
`.claude/agents/` — ported from `.codex/agents/` with the same rules folded
in from `.codex/rules/`:

```text
researcher   → .claude/agents/researcher.md   (read-only: maps code/tests/docs)
architect    → .claude/agents/architect.md    (read-only: design + ADR drafts)
implementer  → .claude/agents/implementer.md  (edits: one plan checkbox)
reviewer     → .claude/agents/reviewer.md     (read-only: independent check)
```

Dispatch each bounded task through the `Agent` tool with the matching
`subagent_type` in this order: researcher → architect → implementer →
reviewer. You (the primary agent) reconcile every handoff with `plan.md` and
record the result in the active task's `AUDIT.md`; mark a plan checkbox
complete only after reviewer findings and verification evidence are
resolved. For small, tightly-scoped work it's fine to fold a role into your
own turn instead of spawning a subagent, but still write the audit entry as
if that role had run.

## Translating remaining Codex-specific instructions

A few conventions in `AGENTS.md`/`.codex/rules/` referred to Codex-only
tooling; the subagents above already use Claude Code's tools instead, but
keep these in mind wherever you edit directly:

- "Use `apply_patch` for edits" → use the `Edit`/`Write` tools; never edit via
  shell redirection, heredocs, or ad-hoc scripts.
- Never use destructive Git commands (`reset --hard`, `clean`, force-push,
  amend/rebase) unless the user explicitly asks.
- Keep `.ai/`, `.codex/`, root `AGENTS.md`, and `index.md` as local context;
  never force-add them or change `.gitignore` policy without an explicit ask.

## Documentation and packaging discipline

After any repository change, review affected `*.md` files (English and
Russian counterparts together), `CHANGELOG.md`, `SUPPORT.md`, `SECURITY.md`,
`CONTRIBUTING.md`, crate `Cargo.toml` metadata, CI YAML, and scripts — per
`.codex/rules/files.md`. Record what was reviewed in the active task's
`AUDIT.md`, even when no edit was needed.

## Verification

Prefer standard Rust workflow: `cargo check`, `cargo test`, `cargo fmt --
--check`, `cargo clippy` as applicable to the crates touched. Report which
commands were run and which were skipped (e.g. platform-specific tests) in
the handoff/audit entry.
