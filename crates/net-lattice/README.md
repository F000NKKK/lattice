# net-lattice

Cross-platform network inspection and configuration through one strongly typed
Rust API. This is the application-facing Net Lattice crate.

## What it provides

- automatic native backend selection on Linux, Windows, and macOS;
- inspection of interfaces, addresses, routes, neighbors, and DNS;
- imperative route, address, and resolver mutation;
- filtered native change monitoring;
- ordered `MutationPlan` execution with runtime validation, cancellation,
  snapshots, explicit compensation, and per-operation reports.

Enable the optional `async` feature for a runtime-independent
`futures::Stream` watcher surface.

## Quick start

```rust,no_run
use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    for interface in lattice.interfaces()? {
        println!("{interface:?}");
    }
    Ok(())
}
```

## Transaction execution

`MutationPlan` is data only. Pass a plan and one `ExecutionOptions` value to
`Lattice::execute_plan`; callbacks can request cancellation, capture prior
state, and perform explicit compensation without multiplying facade methods.
The returned report preserves plan indices and distinguishes validation,
snapshot, execution, cancellation, and compensation boundaries.

## Platform and privilege notes

Read-only APIs are generally unprivileged. Network mutations require the
native privileges and policy allowed by the operating system. Runtime
capabilities describe implemented surfaces, not a guarantee that the current
process is authorized. Prefer a read-after-write check when the operation's
confirmation contract requires it.
