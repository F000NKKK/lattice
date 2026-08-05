# net-lattice-model

Operating-system-independent network domain types for Net Lattice. This crate
models data and contracts; it never inspects or mutates the host system.

## What it provides

- observed interfaces, addresses, routes, neighbors, and DNS configuration;
- partial desired `InterfaceConfig` patches, with a distinct
  `DesiredAdminState` so observed `AdminState::Unknown` is never requested;
- desired static-neighbor intent (`StaticNeighbor`) for ARP/NDP entries,
  distinct from the observed `NeighborEntry` and requiring an explicit MAC;
- desired route-mutation intent (`RouteConfig`), distinct from the observed
  `Route` and carrying no route identifier (no backend accepts one back as
  mutation input); its `metric` field is honored on Linux and Windows only
  and silently ignored on Darwin, matching the observed-side platform gap;
- typed object identifiers and filtered change events;
- mutation descriptions, semantics, snapshots, plans, and execution reports;
- explicit separation between inspectable plan data and runtime execution;
- `CurrentState`, a whole-system observed-state snapshot aggregating routes,
  interfaces, neighbors, interface addresses, and DNS configuration
  (assembled elsewhere, by the facade — this crate only defines the data
  shape and a `CurrentState::new` constructor, since the type is
  `#[non_exhaustive]`);
- `DesiredState`, a whole-system, caller-authored desired-state aggregate
  paralleling `CurrentState`: one `Option`-wrapped field per domain, where
  `None` means the domain is unmanaged and `Some` (including an empty
  collection) means it is managed with that exact desired content. Unlike
  `CurrentState`, it is never assembled from a backend read — build one with
  `DesiredState::empty()` and its `with_routes`/`with_interfaces`/
  `with_neighbors`/`with_addresses`/`with_dns` builder methods;
- `Diff`, the pure, side-effect-free computed difference between a
  `CurrentState` and a `DesiredState`, via
  `Diff::compute(&CurrentState, &DesiredState) -> Diff`: route/neighbor/
  address use a natural-key set-diff (`RouteChange`/`NeighborChange`/
  `AddressChange`, route never producing a `Changed` case), interface uses a
  per-field patch-diff (`InterfaceDiff`) mirroring `InterfaceConfig`'s
  "don't touch" semantics, and DNS is a whole-value comparison (`DnsChange`).
  `Diff::compute` performs no I/O and calls no provider/backend method — it
  does not decide how or whether a diff gets applied.

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

`InterfaceConfig` requires at least one requested setting. Zero is rejected as
an invalid MTU here; all other MTU limits are platform- and interface-specific
backend validation. A configuration patch can describe administrative state,
MTU, or both without manufacturing an observed `Interface`.
