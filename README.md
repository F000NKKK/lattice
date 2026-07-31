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

> **Status:** Net Lattice provides cross-platform network inspection, route and address mutation, and synchronous change monitoring with an optional async interface through native operating-system APIs on Linux, Windows, and macOS. Stage 0.12 of the architecture plan has shipped; see Current Status below.

## Overview

Operating systems expose networking configuration and state through wildly different, low-level, and often platform-specific interfaces: Linux Netlink, the Windows IP Helper API, and macOS BSD routing facilities, among others. Applications that need to inspect or configure networking — IP addresses, routes, interfaces, neighbors, and more — are typically forced to either shell out to external tools, parse text output, or write and maintain separate platform-specific integrations.

Net Lattice aims to unify these interfaces behind a single, strongly typed, idiomatic Rust API, so that consumers never need to deal with raw platform structures, shell commands, or ad hoc string parsing.

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
- DNS resolver inspection
- Neighbor tables (ARP/NDP)
- Network monitoring and change notifications
- Optional runtime-agnostic async event stream

Planned:

- DNS mutation
- VLANs
- VRFs
- Network namespaces
- Firewall integration
- Transactional configuration
- Declarative networking

## Non-Goals

- Net Lattice is not a replacement for full network management daemons (e.g. NetworkManager, systemd-networkd).
- Net Lattice does not aim to provide a command-line interface or GUI as part of the core library.
- Net Lattice does not aim to parse or wrap the output of external CLI tools as a long-term strategy.
- Net Lattice does not aim to support every conceivable network protocol or vendor extension from day one.

## Current Status

Stage 0.12 of the [architecture](ARCHITECTURE.md)'s Incremental Delivery Plan has landed:

- `net-lattice-core`, `net-lattice-ip`
- `net-lattice-model`'s `route`, `mac`, `interface`, `dns`, `neighbor`, `ifaddr`, and `event` modules; `NewInterfaceAddress` expresses address-assignment intent separately from an observed `InterfaceAddress`
- `net-lattice-platform`'s `RouteProvider`, `InterfaceProvider`, `DnsProvider`, `NeighborProvider`, `AddressProvider`, `AddressMutator`, `CapabilityProvider`, synchronous `EventProvider`/bounded `EventReceiver`, and optional async monitoring support
- `net-lattice-async`, which exposes the single runtime-agnostic `EventStream` type
- the `net-lattice` facade, including `Lattice::add_address()`, `Lattice::remove_address()`, `Lattice::capabilities()`, `Lattice::supports()`, `Lattice::watch()`, `Lattice::watch_filtered()`, and feature-gated `Lattice::watch_async()`

This gives real route and interface-address management, interface listing, DNS resolver reads, neighbor (ARP/NDP) table reads, and bounded network-change monitoring on Linux, Windows, and macOS. Address creation accepts `NewInterfaceAddress` and returns the resulting observed `InterfaceAddress`, so callers never invent an address ID. `EventFilter` composes domain selectors (`routes()`) and object selectors (`route(route_id)`); every backend applies the filter before enqueueing an ordinary event. Query `Lattice::supports(Capability::MONITORING)` before watching in portable code; the facade returns `Error::Unsupported` before opening a watcher when monitoring is unavailable. Net Lattice's `async` feature uses and re-exports the `EventStream` implementation from `net-lattice-async`; applications need only enable that facade feature. `Lattice::watch_async(filter)` remains the Stage 0.11 async API and has the same filter semantics as `Lattice::watch_filtered(filter)`. Tokio is used internally where the native implementation requires it, while applications interact only with the runtime-independent `futures::Stream` interface. Platform backends use Netlink on Linux, the IP Helper API on Windows, and BSD routing sockets, `getifaddrs`, and address ioctls on macOS. This is still not a complete library: DNS mutation, VLANs, VRFs, namespaces, firewall integration, transactional configuration, declarative networking, and other advanced capabilities are still ahead; see [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental Delivery Plan for the staged roadmap and [CHANGELOG.md](CHANGELOG.md) for what has actually shipped.

| Capability | Linux | Windows | macOS |
|---|:---:|:---:|:---:|
| Route inspection | ✅ | ✅ | ✅ |
| Route mutation | ✅ | ✅ | ✅ |
| Interface inspection | ✅ | ✅ | ✅ |
| Interface-address inspection | ✅ | ✅ | ✅ |
| Interface-address mutation | ✅ | ✅ | ✅ |
| Neighbor table inspection | ✅ | ✅ | ✅ |
| DNS resolver inspection | ✅ | ✅ | ✅ |
| Change monitoring | ✅ | ✅ | ✅ |
| Async change monitoring | ✅ | ✅ | ✅ |

### Event delivery

Event streams are bounded. If a consumer falls behind, the watcher records and delivers `Event::ResyncRequired { .. }` before a subsequent ordinary event instead of retaining an unbounded backlog. Re-read the affected provider state before relying on subsequent events.

```rust
let route_events = EventFilter::none().route(route_id);
let watcher = lattice.watch_filtered(route_events)?;
```

## Examples

### Inspecting and watching state

```rust
use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    for interface in lattice.interfaces()? {
        println!("{interface:?}");
    }

    for route in lattice.routes()? {
        println!("{route:?}");
    }

    let watcher = lattice.watch()?;
    loop {
        let event = watcher.recv()?;
        println!("{event:?}");
    }
}
```

### Async monitoring

Enable the optional async facade with `net-lattice = { version = "0.12", features = ["async"] }`. It returns the same `futures::Stream` on each supported platform:

```rust
use futures::StreamExt;
use net_lattice::{EventFilter, Lattice, Result};

async fn monitor() -> Result<()> {
    let lattice = Lattice::connect()?;
    let mut events = lattice.watch_async(EventFilter::ALL)?;
    while let Some(event) = events.next().await {
        println!("{:?}", event?);
    }
    Ok(())
}
```

### Assigning an address

Address assignment uses a request type, so callers never construct an observed address ID:

```rust
use net_lattice::{
    Error, Ipv4Address, Ipv4Network, Ipv4PrefixLength, Network, NewInterfaceAddress,
};

let interface = lattice
    .interfaces()?
    .into_iter()
    .next()
    .ok_or(Error::NotFound)?;
let request = NewInterfaceAddress::new(
    interface.id,
    Network::from(Ipv4Network::new(
        Ipv4Address::new(192, 0, 2, 10),
        Ipv4PrefixLength::new(24)?,
    )),
);
let observed = lattice.add_address(request)?;
```

## Roadmap

1. **Bootstrap** *(completed)* — repository infrastructure, licensing, community health files, and tooling configuration.
2. **Design** *(completed)* — define the crate layout, core abstractions, and platform abstraction strategy. See [ARCHITECTURE.md](ARCHITECTURE.md) for the planned workspace structure.
3. **Foundations** *(completed)* — core IP/route/interface types and all three platform backends shipped.
4. **Platform parity** *(completed)* — Linux, Windows, and macOS route and address mutation, interface, DNS-read, neighbor-read, address-read, and monitoring backends shipped.
5. **Stage 0.9: Address mutation** *(completed)* — cross-platform assignment and removal of interface IPv4/IPv6 addresses.
6. **Stage 0.10: Event semantics** *(completed)* — bounded delivery, overflow and resynchronization signaling, filtering, cancellation, and error propagation.
7. **Stage 0.11: Async events** *(completed)* — optional `async` facade feature, one runtime-agnostic `EventStream`, and native Tokio-backed delivery in every platform backend.
8. **Stage 0.12: Watcher API stabilization** *(completed)* — composable object/domain filters, filtering before queueing, monitoring-capability validation, and consistent filter semantics across synchronous and async watchers while preserving the released 0.11 API.
9. **Stage 0.13: DNS mutation** — capability-gated resolver configuration through supported native system mechanisms.
10. **Stage 0.14: Transaction primitives** — plans, application, failure reporting, and rollback boundaries for mutations.
11. **Stage 0.15: Declarative networking** — `CurrentState`, `DesiredState`, `Diff`, and `ApplyPlan` built on the stable mutation foundation.
12. **Stage 0.16+: Capability domains** — VLAN, VRF, namespaces, firewall, and tunnel support, gated by explicit runtime capabilities.
13. **1.0** — stable cross-platform inspection, monitoring, and mutation foundation.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations, and [SECURITY.md](SECURITY.md) for reporting security issues.

## License

Net Lattice is licensed under the [Mozilla Public License 2.0](LICENSE) (`MPL-2.0`).
