# Audit and task decomposition rules

Ported from `.codex/rules/audit.md`.

- Start from one plan checkbox and decompose it into the applicable model,
  platform, facade, backend, test, CI, documentation, and packaging subtasks.
- Every subtask must name its files, public API impact, platform
  assumptions, tests, cleanup behavior, and completion evidence.
- Audit current code before proposing new types or traits; reuse established
  domain types, provider contracts, capabilities, and execution paths.
- Mark a checkbox complete only after implementation, focused tests, docs,
  and verification gates all agree.
- Record unresolved questions in `plan.md`. Write an ADR before choosing a
  public API, cross-crate boundary, compatibility policy, or reversal of an
  accepted decision.
- Keep work limited to the active task plan under `.ai/<task-name>/`.
- Record ongoing evidence in `.ai/<task-name>/AUDIT.md` and architectural or
  public API decisions as `.ai/<task-name>/adr/ADR-NNNN-*.md`.
- Each audit entry must state the date, bounded slice, files/symbols
  inspected, decisions, edits, commands run, results, and remaining risks.
- After each implementation slice, perform a repository-wide documentation
  sync: inspect every relevant `*.md` plus affected extensionless/config
  files such as manifests, ignore rules, CI definitions, and scripts.
