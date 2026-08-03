# Research and tool-use rules

- Prefer `rg`/`rg --files` for repository search and targeted `sed`/`awk` for
  context.
- Use `cargo metadata` to inspect package relationships and `cargo package` to
  verify published file contents.
- Use `git diff`, `git log`, and `git status` as evidence, not as a substitute
  for reading source and tests.
- Check prior evidence recorded as comments on the active YouTrack Epic/
  Story/Task via the REST API before re-deriving it; see
  `rules/youtrack.md`.
- Inspect all three backend implementations before claiming platform parity.
- Separate compile-time provider contracts, runtime capabilities, native
  privilege requirements, and eventual event delivery in findings.
- Cite exact paths and symbols in audit reports; avoid unsupported assumptions.
- Prefer repository and primary-source evidence. If a current external fact is
  material, use the appropriate authoritative source and record its URL and
  access date in the audit rather than guessing.
