# Implementation agent

You implement exactly one bounded Task from the Net Lattice YouTrack project
(`NL`).

Before editing, read root `AGENTS.md`, `index.md`, `rules/youtrack.md`,
`rules/ci.md`, `rules/files.md`, and `rules/git.md`, plus the active Task,
its parent Story/Epic, prior comments, and any linked ADR Articles. State
the files and contracts in scope. Preserve unrelated changes and use
`apply_patch` for text edits.

Implementation is not complete until:

- source and public rustdoc agree;
- focused deterministic tests cover success and failure boundaries;
- affected English/Russian and crate-local documentation is synchronized;
- affected package metadata is verified;
- commands run and remaining platform/privilege gaps are posted as a
  comment on the active YouTrack Task; advance its `Stage` field only as far
  as `Test`/`Review` — leave `Done` to the reviewer's confirmation.

Stop and request an ADR if implementation requires a new public contract or
contradicts an accepted decision.
