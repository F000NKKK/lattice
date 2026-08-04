# Release and versioning rules

- One roadmap Sprint maps to one release version. Sprint `0.18` ships as
  crate version `0.18.x`, Sprint `0.19` ships as `0.19.x`, and so on. The
  YouTrack `Sprint` custom field and the crate's SemVer minor version march
  together pre-1.0 — do not invent a different mapping when filing or
  scoping issues.
- Net Lattice is pre-1.0 (`ARCHITECTURE.md`'s roadmap gates 1.0 behind the
  Stage 0.21 compatibility audit). Before 1.0, a Sprint's release is allowed
  to change the public API meaningfully — add, remove, or reshape public
  types/traits/methods — as normal roadmap evolution, provided the change is
  recorded in an ADR (see `rules/youtrack.md`'s ADR section) and in
  `CHANGELOG.md`. Do not treat a pre-1.0 minor bump as a compatibility
  guarantee, and do not block a design on preserving a pre-1.0 import path
  or type shape purely for compatibility's sake — `ARCHITECTURE.md`'s own
  opening note already states it "reflects intended direction, not current
  state." An ADR may still choose to preserve something pre-1.0 for other
  reasons (ergonomics, avoiding needless churn); the point is that
  compatibility alone is not a blocking constraint before 1.0.
- Once a crate reaches `1.0.0`, this changes: within one major version
  (`1.x`, `2.x`, ...), the public API must not change in a breaking way —
  ordinary SemVer discipline applies (new public items are additive minor
  bumps, fixes are patch bumps; nothing already public may be removed or
  have its behavior/meaning changed without a major version bump). A
  breaking change after 1.0 requires bumping the major version and is a
  deliberate, explicitly user-authorized decision — not something an agent
  introduces quietly inside what looks like a routine Sprint release the way
  pre-1.0 minor bumps can be. Treat any post-1.0 architect/implementer work
  that would break a public contract as requiring the same explicit
  user sign-off a pre-1.0 breaking change gets today, escalated rather than
  assumed.
- `Cargo.toml` version numbers are set by the user, not by an agent. Agents
  never edit a crate's `version` field themselves. If a crate gained public
  API that plausibly warrants a version bump (e.g. a new public type/trait
  was added but the crate's version wasn't bumped), flag that
  version-consistency question explicitly in the relevant issue's evidence
  comment instead of guessing at or editing the number.
- When a crate's version does change (by the user), `CHANGELOG.md`,
  `SECURITY.md`'s supported-version table, and `SUPPORT.md`'s project-status
  summary must be reviewed together per `rules/files.md` — a stale "current
  supported line" statement in `SECURITY.md` or `SUPPORT.md` is exactly the
  kind of drift this rule exists to prevent.
