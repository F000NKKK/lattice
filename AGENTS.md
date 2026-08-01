# Net Lattice agent workflow

This file is the repository entry point for Codex and collaborating agents.
Before changing anything, read:

1. `index.md` for the workspace map and dependency direction;
2. the relevant roadmap sections in `ARCHITECTURE.md` and
   `ARCHITECTURE.ru.md`;
3. `.codex/README.md`, the applicable rules in `.codex/rules/`, and the
   selected role profile in `.codex/agents/`;
4. the active task workspace: logically `./ai/<task-name>/`, stored in this
   repository as `.ai/<task-name>/`.

The active task workspace owns its `plan.md`, `AUDIT.md`, and `adr/` records.
Read them before implementation, update the audit after every bounded slice,
and write an ADR before changing a public contract or reversing an accepted
decision. Never mix unrelated or future work into the active task directory.

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
    ↓ plan/AUDIT reconciliation and handoff
```

1. `researcher` maps existing code, tests, platform behavior, documentation,
   and package metadata. Its audit entry is the input to design.
2. `architect` checks cross-crate boundaries and compatibility, produces the
   smallest design, and writes proposed ADRs. For a purely mechanical change,
   record `architect: not applicable` with the reason in `AUDIT.md`.
3. `implementer` executes one approved plan checkbox. It may not silently
   expand scope or decide a new public contract.
4. `reviewer` performs an independent contract, platform, test, documentation,
   and packaging review. It returns findings to the implementer or clears the
   slice for completion.
5. The primary agent reconciles role outputs with `plan.md`, updates
   `AUDIT.md`, and marks a checkbox complete only after reviewer findings and
   verification evidence are resolved.

Each handoff must identify the active plan checkbox, files/symbols in scope,
decisions already accepted, evidence produced, unresolved risks, and the next
role. Role instructions are defined in `.codex/agents/`; no role may keep its
only record in chat.

Decompose work into the applicable model, platform, facade, backend, test,
CI, documentation, and packaging slices. A task is complete only when source,
tests, rustdoc, user documentation, package metadata, and recorded
verification agree.

After every repository change, review all affected `*.md` files and their
language counterparts. Also inspect affected manifests and extensionless or
configuration files, including `Cargo.toml`, CI definitions, scripts,
`.gitignore`, `SECURITY`, and `SUPPORT` when present. Record files reviewed in
the active `AUDIT.md`, even when no edit was required.

Preserve unrelated user changes, use `apply_patch` for text edits, and never
use destructive Git commands. Repository-local agent files are working
context; do not force-add ignored files or change ignore policy unless the
user explicitly requests it.
