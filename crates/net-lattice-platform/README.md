# net-lattice-platform

Generic provider traits and runtime capability contracts between Net Lattice's
public facade and native platform backends.

## What it provides

- generic inspection and mutation provider traits using associated types;
- runtime `Capability` reporting;
- synchronous event sender/receiver contracts;
- optional native Tokio watcher contracts behind the `async` feature.

This crate intentionally depends on `net-lattice-core`, not
`net-lattice-model`. The facade binds provider associated types to the
workspace's concrete domain model. Application code normally uses
`net-lattice`; backend authors depend on this crate directly.

## Usage

```rust
use net_lattice_platform::{Capability, CapabilityProvider};

fn supports_monitoring<P: CapabilityProvider>(provider: &P) -> bool {
    provider.capabilities().contains(Capability::MONITORING)
}
```

## Contract notes

Capabilities report implemented runtime surfaces. They do not guarantee that
the current process has native privileges or that state cannot change between
validation and submission.
