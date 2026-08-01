# Net Lattice Codex configuration

This directory contains reusable repository workflow, not the state of one
roadmap stage. Task-specific plans, evidence, and decisions live under
`.ai/<task-name>/`.

## Load order

1. Read root `AGENTS.md` and `index.md`.
2. Identify the active `.ai/<task-name>/` directory.
3. Read its `plan.md`, `AUDIT.md`, and relevant ADRs.
4. Load every applicable rule from `rules/`.
5. Follow the root pipeline through `researcher`, `architect`, `implementer`,
   and `reviewer`. Record an explicit `not applicable` audit decision when a
   mechanical slice does not need architecture work.
6. Let the primary agent reconcile every handoff with the plan and record the
   result in the active task workspace.

## Contents

- `config.toml` — minimal repository-local Codex discovery settings.
- `rules/` — reusable audit, file, Git, research, and CI constraints.
- `agents/` — role profiles for research, design, implementation, and review.
- `templates/` — starting structures for a new task plan, audit log, and ADR.

## New task workspace

Create `.ai/<task-name>/` from the templates. The directory must contain:

```text
.ai/<task-name>/
├── plan.md
├── AUDIT.md
└── adr/
    ├── README.md
    └── ADR-NNNN-short-title.md
```

The plan is the authoritative TODO. `AUDIT.md` records what was inspected,
changed, and verified. ADRs record decisions, alternatives, and consequences;
they do not replace public rustdoc, architecture, or user documentation.

## Handoff contract

Every role appends an audit entry containing its role, plan checkbox,
files/symbols inspected, output, commands, unresolved risks, and next role.
The reviewer must not reuse the implementer's claim as evidence: it inspects
the diff and verification results independently.
