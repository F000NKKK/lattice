# net-lattice

Cross-platform network inspection and configuration through one strongly typed
Rust API. This is the application-facing Net Lattice crate.

## What it provides

- automatic native backend selection on Linux, Windows, and macOS;
- inspection of interfaces, addresses, routes, neighbors, and DNS;
- `current_state()`, a single call returning a whole-system `CurrentState`
  snapshot (routes, interfaces, neighbors, addresses, and DNS) assembled from
  the same per-domain reads, with zero extra backend code required;
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

## Whole-system snapshot

`current_state()` reads routes, interfaces, neighbors, addresses, and DNS in
one call and returns them as a single `CurrentState`. Each domain is still an
independent backend read — there is no lock or transaction spanning them, so
treat the result as several closely timed reads, not one atomic capture. If
any one read fails, the whole call fails and returns no partial state.

```rust,no_run
use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    let state = lattice.current_state()?;
    println!("{} routes, {} interfaces", state.routes.len(), state.interfaces.len());
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
use net_lattice::mutation::{DesiredAdminState, InterfaceConfig};
use net_lattice::{Capability, Error, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    // Replace "eth0" with the actual name of the interface you intend to
    // change — never select the first/loopback interface for a mutation.
    let target_name = "eth0";
    let interface = lattice
        .interfaces()?
        .into_iter()
        .find(|interface| interface.name == target_name)
        .ok_or(Error::NotFound)?;

    if lattice.supports(Capability::INTERFACE_ADMIN_STATE) {
        let config = InterfaceConfig::new(interface.id, Some(DesiredAdminState::Up), None)?;
        let observed = lattice.set_interface_config(config)?;
        println!("{observed:?}");
    }
    Ok(())
}
```

When one patch asks for both MTU and administrative state, a native backend
may use separate writes. Treat errors as potentially partially applied and use
an explicit `MutationPlan` compensator if restoration is needed.

## Static neighbor mutation

`StaticNeighbor` is desired intent, distinct from the observed `NeighborEntry`:
it carries neither the synthesized `NeighborId` nor the observed
`NeighborState`, and requires a MAC address because only static L2 mappings
can be created this way. Check `Capability::NEIGHBOR_MUTATION` first. A
successful add returns the observed entry read back from the OS; removing a
present but non-`Permanent` (dynamically learned) entry is refused with
`Error::InvalidState` rather than silently evicting it.

```rust,no_run
use net_lattice::model::{IpAddress, MacAddress};
use net_lattice::mutation::StaticNeighbor;
use net_lattice::{Capability, Error, Ipv4Address, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    // Replace "eth0" with the actual name of the interface you intend to
    // change — never select the first/loopback interface for a mutation.
    let target_name = "eth0";
    let interface = lattice
        .interfaces()?
        .into_iter()
        .find(|interface| interface.name == target_name)
        .ok_or(Error::NotFound)?;

    if lattice.supports(Capability::NEIGHBOR_MUTATION) {
        let neighbor = StaticNeighbor::new(
            interface.id,
            IpAddress::from(Ipv4Address::new(192, 0, 2, 250)),
            MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0xfa]),
        );
        let observed = lattice.add_static_neighbor(neighbor)?;
        println!("{observed:?}");
    }
    Ok(())
}
```

## Monitoring capabilities

Select only event domains the connected backend advertises. The aggregate
`Capability::MONITORING` means all route, interface, neighbor, and address
domains are deliverable, so it is required by `watch()`. For a filtered watch,
check the matching domain capability. Windows supports route, interface, and
address notifications, but rejects neighbor and all-domain subscriptions with
`Error::Unsupported` rather than returning a stream that omits neighbors.

```rust,no_run
use net_lattice::monitoring::EventFilter;
use net_lattice::{Capability, Lattice, Result};

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
