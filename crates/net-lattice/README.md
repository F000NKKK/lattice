# net-lattice

Cross-platform network inspection and configuration through one strongly typed
Rust API. This is the application-facing Net Lattice crate.

## What it provides

- automatic native backend selection on Linux, Windows, and macOS;
- inspection of interfaces, addresses, routes, neighbors, and DNS;
- imperative route, address, resolver, and static ARP/NDP neighbor mutation;
- partial interface MTU and administrative-state configuration;
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

## Interface configuration

`InterfaceConfig` is desired intent, distinct from the observed `Interface`.
Build a patch with at least one requested setting, check the matching runtime
capabilities, then submit it. A successful call returns an observed interface
read after the native update.

```rust,no_run
use net_lattice::{Capability, DesiredAdminState, InterfaceConfig, InterfaceId, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    if lattice.supports(Capability::INTERFACE_ADMIN_STATE) {
        let config = InterfaceConfig::new(
            InterfaceId::new(7),
            Some(DesiredAdminState::Up),
            None,
        )?;
        let observed = lattice.set_interface_config(config)?;
        println!("{observed:?}");
    }
    Ok(())
}
```

When one patch asks for both MTU and administrative state, a native backend
may use separate writes. Treat errors as potentially partially applied and use
an explicit `MutationPlan` compensator if restoration is needed.

## Monitoring capabilities

Select only event domains the connected backend advertises. The aggregate
`Capability::MONITORING` means all route, interface, neighbor, and address
domains are deliverable, so it is required by `watch()`. For a filtered watch,
check the matching domain capability. Windows supports route, interface, and
address notifications, but rejects neighbor and all-domain subscriptions with
`Error::Unsupported` rather than returning a stream that omits neighbors.

```rust,no_run
use net_lattice::{Capability, EventFilter, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    if lattice.supports(Capability::ROUTE_MONITORING) {
        let _routes = lattice.watch_filtered(EventFilter::none().routes())?;
    }
    Ok(())
}
```

## Platform and privilege notes

Read-only APIs are generally unprivileged. Network mutations require the
native privileges and policy allowed by the operating system. Runtime
capabilities describe implemented surfaces, not a guarantee that the current
process is authorized. Prefer a read-after-write check when the operation's
confirmation contract requires it.
