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
