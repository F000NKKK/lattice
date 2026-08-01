# net-lattice-platform

Generic provider traits and runtime capability contracts between Net Lattice's
domain model and native platform backends. This crate intentionally depends
only on `net-lattice-core`, not on `net-lattice-model`.

It defines inspection, mutation, monitoring, async delivery, and capability
interfaces implemented by the platform backend crates.

## Example contract

```rust
use net_lattice_platform::{Capability, CapabilityProvider};

fn supports_monitoring<P: CapabilityProvider>(provider: &P) -> bool {
    provider.capabilities().contains(Capability::MONITORING)
}
```
