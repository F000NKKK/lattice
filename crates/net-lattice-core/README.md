# net-lattice-core

Shared foundation types for the Net Lattice workspace. This crate is
platform-independent and performs no operating-system or network I/O.

## What it provides

- `Error` and `Result<T>` used across every Net Lattice crate;
- `PlatformErrorCode` for preserving native Linux, Windows, and macOS errors;
- `Id<T>`, a strongly typed identifier that prevents mixing object domains.

Most applications should use these types through the `net-lattice` facade.
Depend on this crate directly when implementing a backend or a library that
shares Net Lattice's foundational contracts.

## Usage

```rust
use net_lattice_core::{Id, Result};

struct Interface;

fn selected_interface() -> Result<Id<Interface>> {
    Ok(Id::new(7))
}

assert_eq!(selected_interface().unwrap().value(), 7);
```

`Id<Route>` and `Id<Interface>` remain different Rust types even when their
numeric values match.
