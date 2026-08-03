# Test and CI rules

- Ordinary tests must be deterministic, non-privileged, and non-destructive.
- Privileged tests must be `#[ignore]` or isolated in privileged CI jobs,
  save original state, and restore it on every exit path.
- Test Linux, Windows, and macOS native behavior separately; a passing Linux
  test does not establish platform parity.
- Derive the behavior matrix from the active plan. Normally cover success,
  invalid input, missing objects, capability rejection, native failure,
  partial application, read-after-write, cancellation, snapshot failure,
  compensation, and relevant event filtering.
- Run formatting, workspace tests, clippy, docs, package listing, and diff
  checks before advancing a YouTrack Task's `Stage` field to `Done`.
- Run package listings for every crate whose manifest, README, features, or
  public dependencies changed; verify the archive contains its local README.
- Never relax coverage or lint policy as a substitute for missing behavior;
  add focused tests or document a deliberate platform limitation.
