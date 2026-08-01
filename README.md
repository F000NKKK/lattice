# Net Lattice

**Languages**

🇺🇸 **English** | 🇷🇺 [Русский](README.ru.md)

[![License: MPL 2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![crates.io](https://img.shields.io/crates/v/net-lattice.svg)](https://crates.io/crates/net-lattice)
[![docs.rs](https://img.shields.io/docsrs/net-lattice)](https://docs.rs/net-lattice)
[![Downloads](https://img.shields.io/crates/d/net-lattice.svg)](https://crates.io/crates/net-lattice)
[![MSRV](https://img.shields.io/badge/MSRV-1.93-lightgrey.svg)](Cargo.toml)

![Linux](https://img.shields.io/badge/Linux-supported-success)
![Windows](https://img.shields.io/badge/Windows-supported-success)
![macOS](https://img.shields.io/badge/macOS-supported-success)

**Net Lattice** is a modern, cross-platform Rust library for configuring and inspecting operating system networking through a single, strongly typed API.

> **Status:** Net Lattice provides cross-platform network inspection, route, address, and DNS mutation, inspectable mutation plans, and ordered transaction execution with cancellation, snapshots, compensation, and phase-aware reports. Stage 0.15 of the architecture plan has shipped; see Current Status below.

## Overview

Operating systems expose networking configuration and state through wildly different, low-level, and often platform-specific interfaces: Linux Netlink, the Windows IP Helper API, and macOS BSD routing facilities, among others. Applications that need to inspect or configure networking — IP addresses, routes, interfaces, neighbors, and more — are typically forced to either shell out to external tools, parse text output, or write and maintain separate platform-specific integrations.

Net Lattice aims to unify these interfaces behind a single, strongly typed, idiomatic Rust API, so that consumers never need to deal with raw platform structures, shell commands, or ad hoc string parsing.

## Workspace crates

The published workspace is split into focused crates. Each crate has its own
crate-level README with its scope and a usage example:

| Crate | Purpose |
|---|---|
| [`net-lattice`](crates/net-lattice/README.md) | Public facade and transaction executor |
| [`net-lattice-model`](crates/net-lattice-model/README.md) | Observed state, intent, events, and mutation plans |
| [`net-lattice-platform`](crates/net-lattice-platform/README.md) | Provider and capability contracts |
| [`net-lattice-core`](crates/net-lattice-core/README.md) | Shared errors, results, and IDs |
| [`net-lattice-ip`](crates/net-lattice-ip/README.md) | IPv4/IPv6 addresses and networks |
| [`net-lattice-async`](crates/net-lattice-async/README.md) | Runtime-independent event stream adapter |
| [`net-lattice-backend-linux`](crates/net-lattice-backend-linux/README.md) | Linux Netlink backend |
| [`net-lattice-backend-windows`](crates/net-lattice-backend-windows/README.md) | Windows IP Helper backend |
| [`net-lattice-backend-darwin`](crates/net-lattice-backend-darwin/README.md) | macOS BSD/PF_ROUTE backend |

## Motivation

Cross-platform networking tooling in the Rust ecosystem is fragmented. Existing solutions are frequently platform-specific, incomplete, or built around shelling out to system utilities such as `ip`, `netsh`, or `route`. This is fragile, hard to test, and unsuitable for building robust, production-grade network management software.

Net Lattice is intended to fill this gap by providing a single, well-designed abstraction layer over native OS networking APIs.

## Philosophy

- **Strong typing over strings.** Consumers interact with typed Rust values — addresses, prefixes, routes, interfaces — never raw strings or shell commands.
- **Native APIs, not subprocesses.** Net Lattice talks directly to platform networking APIs (Netlink, IP Helper API, route sockets) rather than invoking external CLI tools.
- **Cross-platform by design.** A single API surface backed by platform-specific implementations, so applications do not need to special-case operating systems.
- **Correctness and safety first.** Networking configuration is sensitive; the library should make incorrect states difficult to represent.
- **Incremental, well-considered growth.** Features are added deliberately, with attention to API design and long-term maintainability, rather than rushed to cover every possible use case.

## Capabilities

Implemented:

- IPv4/IPv6 address and prefix types
- Interface-address inspection and mutation
- Route inspection and mutation
- Interface inspection
- DNS resolver inspection and mutation
- Inspectable mutation plans for routes, addresses, and DNS
- Ordered mutation-plan execution with cancellation, snapshots, explicit
  compensation, and phase-aware reports
- Neighbor tables (ARP/NDP)
- Network monitoring and change notifications
- Optional runtime-agnostic async event stream

Planned:

- VLANs
- VRFs
- Network namespaces
- Firewall integration
- Declarative networking

## Non-Goals

- Net Lattice is not a replacement for full network management daemons (e.g. NetworkManager, systemd-networkd).
- Net Lattice does not aim to provide a command-line interface or GUI as part of the core library.
- Net Lattice does not aim to parse or wrap the output of external CLI tools as a long-term strategy.
- Net Lattice does not aim to support every conceivable network protocol or vendor extension from day one.

## Current Status

Stage 0.15 of the [architecture](ARCHITECTURE.md)'s Incremental Delivery Plan has landed:

- `net-lattice-core`, `net-lattice-ip`
- `net-lattice-model`'s `route`, `mac`, `interface`, `dns`, `neighbor`, `ifaddr`, `event`, and `mutation` modules; `NewInterfaceAddress` and `NewDnsConfig` express mutation intent separately from observed state
- `net-lattice-platform`'s `RouteProvider`, `InterfaceProvider`, `DnsProvider`, `DnsMutator`, `NeighborProvider`, `AddressProvider`, `AddressMutator`, `CapabilityProvider`, synchronous `EventProvider`/bounded `EventReceiver`, and optional async monitoring support
- `net-lattice-async`, which exposes the single runtime-agnostic `EventStream` type
- the `net-lattice` facade, including `Lattice::add_address()`, `Lattice::remove_address()`, `Lattice::set_dns_config()`, `Lattice::capabilities()`, `Lattice::supports()`, `Lattice::watch()`, `Lattice::watch_filtered()`, `Lattice::execute_plan()`, and feature-gated `Lattice::watch_async()`

This gives real route and interface-address management, interface listing, DNS resolver inspection and mutation, neighbor (ARP/NDP) table reads, inspectable mutation plans, ordered transaction execution, and bounded network-change monitoring on Linux, Windows, and macOS. Address creation accepts `NewInterfaceAddress` and returns the resulting observed `InterfaceAddress`; resolver replacement accepts `NewDnsConfig` and returns the resulting observed `DnsConfig`. `MutationPlan` is data only: it exposes operation semantics, while `Lattice::execute_plan` applies a plan through one `ExecutionOptions` value with runtime validation, cancellation boundaries, typed snapshots, explicit compensation, and phase-aware reports. `EventFilter` composes domain selectors (`routes()`) and object selectors (`route(route_id)`); every backend applies the filter before enqueueing an ordinary event. Query `Lattice::supports(Capability::MONITORING)` before watching and `Lattice::supports(Capability::DNS_MUTATION)` before replacing resolver configuration in portable code. Unix resolver managers may later regenerate `/etc/resolv.conf`; callers requiring persistence should use the owning manager's configuration interface. Net Lattice's `async` feature uses and re-exports the `EventStream` implementation from `net-lattice-async`; applications need only enable that facade feature. `Lattice::watch_async(filter)` remains the Stage 0.11 async API and has the same filter semantics as `Lattice::watch_filtered(filter)`. Tokio is used internally where the native implementation requires it, while applications interact only with the runtime-independent `futures::Stream` interface. Platform backends use Netlink on Linux, the IP Helper API on Windows, and BSD routing sockets, `getifaddrs`, and address ioctls on macOS. This is still not a complete library: VLANs, VRFs, namespaces, firewall integration, declarative networking, and other advanced capabilities are still ahead; see [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental Delivery Plan for the staged roadmap and [CHANGELOG.md](CHANGELOG.md) for what has actually shipped.

| Capability | Linux | Windows | macOS |
|---|:---:|:---:|:---:|
| Route inspection | ✅ | ✅ | ✅ |
| Route mutation | ✅ | ✅ | ✅ |
| Interface inspection | ✅ | ✅ | ✅ |
| Interface-address inspection | ✅ | ✅ | ✅ |
| Interface-address mutation | ✅ | ✅ | ✅ |
| Neighbor table inspection | ✅ | ✅ | ✅ |
| DNS resolver inspection | ✅ | ✅ | ✅ |
| DNS resolver mutation | ✅ | ✅ | ✅ |
| Change monitoring | ✅ | ✅ | ✅ |
| Async change monitoring | ✅ | ✅ | ✅ |

### Event delivery

Event streams are bounded. If a consumer falls behind, the watcher records and delivers `Event::ResyncRequired { .. }` before a subsequent ordinary event instead of retaining an unbounded backlog. Re-read the affected provider state before relying on subsequent events.

```rust
let route_events = EventFilter::none().route(route_id);
let watcher = lattice.watch_filtered(route_events)?;
```

## Examples

The runnable sources in [`crates/net-lattice/examples`](crates/net-lattice/examples)
cover every currently available facade operation. Read-only examples are safe to
run; mutation examples require an explicit environment-variable opt-in and
elevated operating-system privilege.

| Scenario | Runnable example | Facade/API covered |
|---|---|---|
| Complete read-only state | [`snapshot`](crates/net-lattice/examples/snapshot.rs) | `capabilities`, `interfaces`, `routes`, `addresses`, `dns_config`, `neighbors` |
| Runtime feature selection | [`capabilities`](crates/net-lattice/examples/capabilities.rs) | `capabilities`, `supports`, every current `Capability` flag |
| Focused route read | [`list_routes`](crates/net-lattice/examples/list_routes.rs) | `routes` |
| Bounded synchronous delivery | [`sync_monitor`](crates/net-lattice/examples/sync_monitor.rs) | `watch`, `recv_timeout`, `Event::ResyncRequired` |
| Domain and object filtering | [`filtered_monitor`](crates/net-lattice/examples/filtered_monitor.rs) | `watch_filtered`, every `EventFilter` domain and object selector |
| Native async delivery | [`async_monitor`](crates/net-lattice/examples/async_monitor.rs) | `watch_async`, `EventStream` |
| Address lifecycle | [`address_assignment`](crates/net-lattice/examples/address_assignment.rs) | `NewInterfaceAddress`, `add_address`, `remove_address` |
| Route lifecycle | [`route_mutation`](crates/net-lattice/examples/route_mutation.rs) | `Route`, `add_route`, `remove_route` |
| Resolver replacement | [`dns_mutation`](crates/net-lattice/examples/dns_mutation.rs) | `NewDnsConfig`, `set_dns_config`, read-after-write verification |
| Mutation inspection | [`mutation_plan`](crates/net-lattice/examples/mutation_plan.rs) | every `Mutation` variant, `Mutation::semantics`, `MutationPlan` |

Run an example with `cargo run -p net-lattice --example <name>`. Add
`--features async` for `async_monitor`.

For a compact application-facing walkthrough, use the
[`net-lattice` crate README](crates/net-lattice/README.md). The other crate
guides in the workspace table document direct library and backend use without
duplicating those contracts here.

## Roadmap

1. **Bootstrap** *(completed)* — repository infrastructure, licensing, community health files, and tooling configuration.
2. **Design** *(completed)* — define the crate layout, core abstractions, and platform abstraction strategy. See [ARCHITECTURE.md](ARCHITECTURE.md) for the planned workspace structure.
3. **Foundations** *(completed)* — core IP/route/interface types and all three platform backends shipped.
4. **Platform parity** *(completed)* — Linux, Windows, and macOS route and address mutation, interface, DNS-read, neighbor-read, address-read, and monitoring backends shipped.
5. **Stage 0.9: Address mutation** *(completed)* — cross-platform assignment and removal of interface IPv4/IPv6 addresses.
6. **Stage 0.10: Event semantics** *(completed)* — bounded delivery, overflow and resynchronization signaling, filtering, cancellation, and error propagation.
7. **Stage 0.11: Async events** *(completed)* — optional `async` facade feature, one runtime-agnostic `EventStream`, and native Tokio-backed delivery in every platform backend.
8. **Stage 0.12: Watcher API stabilization** *(completed)* — composable object/domain filters, filtering before queueing, monitoring-capability validation, and consistent filter semantics across synchronous and async watchers while preserving the released 0.11 API.
9. **Stage 0.13: DNS mutation** *(completed)* — capability-gated resolver replacement through supported system mechanisms on Linux, Windows, and macOS.
10. **Stage 0.14: Mutation operation model** *(completed)* — inspectable `Mutation` values and data-only `MutationPlan`s for existing route, address, and DNS mutations; preconditions, idempotency, privilege, confirmation, partial-application, and reversibility are explicit.
11. **Stage 0.15: Transaction execution** *(completed)* — ordered plans, per-operation outcomes, phase/timing diagnostics, cancellation and failure boundaries, plus compensation only for documented reversible operations.
12. **Stage 0.16: Interface configuration** — separate desired interface configuration, capability-gated admin-state and MTU mutation, and platform-parity tests.
13. **Stage 0.17: Neighbor mutation** — intent/observed types and capability-gated static ARP/NDP management.
14. **Stage 0.18: Snapshots** — consistently assembled `CurrentState` with explicit scope, consistency, and partial-read semantics.
15. **Stage 0.19: Declarative diff** — separate `DesiredState` configuration types and an inspectable `Diff`, without mutation.
16. **Stage 0.20: Declarative apply** — compile a `Diff` into an `ApplyPlan` and execute it through the transaction engine.
17. **Stage 0.21: Pre-1.0 hardening** — freeze public contracts, identity and capability rules, event guarantees, platform matrix, and privileged regression coverage.
18. **Stage 0.22+: Capability domains** — VLAN, VRF, namespaces, firewall, and tunnels, each with a complete read/intent/mutation/event/capability/test contract. They are not prerequisites for 1.0.
19. **1.0** — stable foundation for the implemented inspection, monitoring, imperative mutation, transaction, and declarative-apply contracts. It is gated by the 0.21 compatibility audit, not by every future network domain.

Stages are delivery boundaries, not a promise of one release per heading: platform validation may split a stage, and focused hardening releases may appear between stages.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations, and [SECURITY.md](SECURITY.md) for reporting security issues.

## License

Net Lattice is licensed under the [Mozilla Public License 2.0](LICENSE) (`MPL-2.0`).
