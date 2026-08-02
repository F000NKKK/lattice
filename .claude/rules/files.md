# File and documentation rules

- Read the applicable `AGENTS.md`, `plan.md`, and `index.md` before editing.
- Edit with the `Edit`/`Write` tools. Do not write files with shell
  redirection, heredocs, Python scripts, or ad-hoc generators.
- Preserve established domain distinctions such as observed state versus
  desired intent; the active plan and ADRs define task-specific names.
- Public types, traits, methods, capability flags, and enum variants require
  rustdoc and exports from the intended facade/module.
- Update English and Russian README/architecture documents together when the
  changed concept appears in both.
- Changes to behavior also require CHANGELOG, SUPPORT, SECURITY, and
  CONTRIBUTING review when their status or support statements are affected.
- Do not edit generated `target/` content or include `.ai/` working records
  in published crate sources.
- Each crate has its own local `README.md`; crate READMEs must not use
  relative links to the repository root because crates.io does not support
  those links.
- A crate README must stand alone on crates.io: explain purpose, intended
  audience, main surface, a valid usage example, feature/platform
  constraints, and privilege or safety requirements where applicable.
- After every repository change, scan and synchronize all relevant `*.md`
  files, including English/Russian counterparts, changelog, support,
  security, architecture, and contribution docs. Also inspect relevant
  extensionless project files such as `Cargo.toml`, `SECURITY`, `SUPPORT`,
  `.gitignore`, CI YAML, and scripts when the change affects packaging,
  workflow, or policy.
- Documentation review is complete only when no stale version, roadmap,
  feature, API, package-metadata, or support statement remains in the
  affected files. Record what was reviewed in the active task's `AUDIT.md`,
  even when no edit was needed.
