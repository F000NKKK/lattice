# net-lattice-model

Operating-system-independent network domain types for Net Lattice. This crate
models data and contracts; it never inspects or mutates the host system.

## What it provides

- observed interfaces, addresses, routes, neighbors, and DNS configuration;
- typed object identifiers and filtered change events;
- mutation descriptions, semantics, snapshots, plans, and execution reports;
- explicit separation between inspectable plan data and runtime execution.

Use this crate directly for offline plan construction, policy analysis,
serialization layers, or backend development. Use the `net-lattice` facade to
connect these models to an operating system.

## Usage

```rust
use net_lattice_model::{EventFilter, InterfaceId};

let filter = EventFilter::none().interface(InterfaceId::new(7));
assert!(!filter.is_empty());
```

## Contract notes

`MutationPlan::preflight` is static and side-effect free. Runtime capability,
privilege, and object-state validation belongs to an executor such as the one
provided by the `net-lattice` facade.
