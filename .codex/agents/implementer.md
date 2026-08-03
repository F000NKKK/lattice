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

If the user has asked for a commit, name the active Task's `NL-*` ID (and any
other `NL-*` ID it relates to) per `rules/git.md` — never commit without it.
If you are applying a decision recorded in an ADR Article (`NL-A-*` under
`NL-A-1`), also name that Article ID in the commit message.
