# net-lattice-core

Shared foundation types for Net Lattice: the workspace-wide `Error`/`Result`
model, platform-tagged error codes, and typed identifiers.

This crate has no operating-system dependency and is used by the IP, model,
platform, backend, and facade crates.

The published `net-lattice` facade provides the user-facing API.

## Example

```rust
use net_lattice_core::{Error, Result};

fn lookup() -> Result<()> {
    Err(Error::NotFound)
}
```
