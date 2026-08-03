# Net Lattice agent workflow

This file is the repository entry point for Codex and collaborating agents.
Before changing anything, read:

1. `index.md` for the workspace map and dependency direction;
2. the relevant roadmap sections in `ARCHITECTURE.md` and
   `ARCHITECTURE.ru.md`;
3. `.codex/README.md`, the applicable rules in `.codex/rules/`, and the
   selected role profile in `.codex/agents/`;
4. the active work item in the YouTrack project `NL`
   (https://hush.youtrack.cloud/projects/NL) — its Epic (roadmap stage) and
   the current Task/Story, reached via the REST API per
   `.codex/rules/youtrack.md`.

The active Task/Story owns its description, comment-based audit trail, and
any linked ADR Articles (under `NL-A-1`). Read them before implementation,
post a comment after every bounded slice, and write an ADR Article before
changing a public contract or reversing an accepted decision. Never mix
unrelated or future work into the active Task. The former file-based
`.ai/<task-name>/` workspace is retired — do not recreate it.

## Agent pipeline

Run every bounded task through the repository roles in this order:

```text
researcher
    ↓ evidence and contract gaps
architect
    ↓ design, diagrams, and ADRs when required
implementer
    ↓ source, tests, rustdoc, docs, and package metadata
reviewer
    ↓ independent findings and verification gate
primary agent
    ↓ YouTrack reconciliation and handoff
```

1. `researcher` maps existing code, tests, platform behavior, documentation,
   and package metadata. Its comment on the active issue is the input to
   design.
2. `architect` checks cross-crate boundaries and compatibility, produces the
   smallest design, and drafts proposed ADR Articles. For a purely
   mechanical change, post `architect: not applicable` with the reason as a
   comment.
3. `implementer` executes one approved Task. It may not silently expand
   scope or decide a new public contract.
4. `reviewer` performs an independent contract, platform, test,
   documentation, and packaging review. It returns findings to the
   implementer or clears the slice for completion.
5. The primary agent reconciles role outputs with the active Task, posts the
   consolidated evidence, and advances `Stage` to `Done` only after reviewer
   findings and verification evidence are resolved.

Each handoff must identify the active Task, files/symbols in scope,
decisions already accepted, evidence produced, unresolved risks, and the
next role. Role instructions are defined in `.codex/agents/`; no role may
keep its only record in chat.

Decompose work into the applicable model, platform, facade, backend, test,
CI, documentation, and packaging slices. A task is complete only when source,
tests, rustdoc, user documentation, package metadata, and recorded
verification agree.

After every repository change, review all affected `*.md` files and their
language counterparts. Also inspect affected manifests and extensionless or
configuration files, including `Cargo.toml`, CI definitions, scripts,
`.gitignore`, `SECURITY`, and `SUPPORT` when present. Record files reviewed
as a comment on the active YouTrack issue, even when no edit was required.

Preserve unrelated user changes, use `apply_patch` for text edits, and never
use destructive Git commands. Repository-local agent files are working
context; do not force-add ignored files or change ignore policy unless the
user explicitly requests it.
