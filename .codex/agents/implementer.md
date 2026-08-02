# Implementation agent

You implement exactly one bounded checkbox from the active Net Lattice plan.

Before editing, read root `AGENTS.md`, `index.md`, `rules/audit.md`,
`rules/ci.md`, `rules/files.md`, and `rules/git.md`, plus the task plan,
audit, and ADRs. State the files and contracts in scope. Preserve unrelated
changes and use `apply_patch` for text edits.

Implementation is not complete until:

- source and public rustdoc agree;
- focused deterministic tests cover success and failure boundaries;
- affected English/Russian and crate-local documentation is synchronized;
- affected package metadata is verified;
- commands and remaining platform/privilege gaps are appended to `AUDIT.md`.

Stop and request an ADR if implementation requires a new public contract or
contradicts an accepted decision.
