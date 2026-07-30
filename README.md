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

> **Status:** Net Lattice has shipped through Stage 0.8 of its architecture plan. The repository contains real implementations for listing, adding, and removing IPv4/IPv6 routes; listing network interfaces; reading DNS resolver configuration, neighbor (ARP/NDP) tables, and IP addresses assigned to interfaces; and monitoring network changes on Linux, Windows, and BSD/macOS. This is still a minimal vertical slice, not a complete library — see Current Status below.

## Overview

Operating systems expose networking configuration and state through wildly different, low-level, and often platform-specific interfaces: Linux Netlink, the Windows IP Helper API, BSD/macOS route sockets, and various vendor-specific mechanisms. Applications that need to inspect or configure networking — IP addresses, routes, interfaces, neighbors, and more — are typically forced to either shell out to external tools, parse text output, or write and maintain separate platform-specific integrations.

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

## Long-Term Goals

Net Lattice intends to eventually provide support for:

- IP addresses
- Network prefixes
- Routes
- Interfaces
- Gateways
- DNS configuration
- Neighbor tables (ARP/NDP)
- VLANs
- VRFs
- Network namespaces
- Firewall integration
- Network monitoring and change notifications
- Transactional configuration
- Declarative networking

## Non-Goals

- Net Lattice is not a replacement for full network management daemons (e.g. NetworkManager, systemd-networkd).
- Net Lattice does not aim to provide a command-line interface or GUI as part of the core library.
- Net Lattice does not aim to parse or wrap the output of external CLI tools as a long-term strategy.
- Net Lattice does not aim to support every conceivable network protocol or vendor extension from day one.

## Current Status

Stage 0.8 of the [architecture](ARCHITECTURE.md)'s Incremental Delivery Plan has landed:

- `net-lattice-core`, `net-lattice-ip`
- `net-lattice-model`'s `route`, `mac`, `interface`, `dns`, `neighbor`, `ifaddr`, and `event` modules
- `net-lattice-platform`'s `RouteProvider`, `InterfaceProvider`, `DnsProvider`, `NeighborProvider`, `AddressProvider`, `CapabilityProvider`, and synchronous `EventProvider`/`EventReceiver`
- `net-lattice-backend-linux` (routes, interfaces, neighbors, addresses, and monitoring via Netlink; DNS via `/etc/resolv.conf`)
- `net-lattice-backend-windows` (routes and interfaces via the Windows IP Helper API, DNS via `GetAdaptersAddresses`, neighbors via `GetIpNetTable2`, addresses via `GetUnicastIpAddressTable`, monitoring via IP Helper notifications)
- `net-lattice-backend-darwin` (routes, neighbors, addresses, and monitoring via BSD/macOS route sockets/`getifaddrs`, interfaces via `getifaddrs`, DNS via `/etc/resolv.conf`)
- the `net-lattice` facade, including `Lattice::capabilities()`, `Lattice::supports()`, and `Lattice::watch()`

This gives real route management, interface listing, DNS resolver reads, neighbor (ARP/NDP) table reads, interface address reads, and network-change monitoring on Linux, Windows, and BSD/macOS. Query `Lattice::supports(Capability::MONITORING)` before calling `Lattice::watch()` in portable code. This is still not a complete library: every other item in the Long-Term Goals above is still ahead; see [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental Delivery Plan for the staged roadmap and [CHANGELOG.md](CHANGELOG.md) for what has actually shipped.

## Roadmap

1. **Bootstrap** *(completed)* — repository infrastructure, licensing, community health files, and tooling configuration.
2. **Design** *(completed)* — define the crate layout, core abstractions, and platform abstraction strategy. See [ARCHITECTURE.md](ARCHITECTURE.md) for the planned workspace structure.
3. **Foundations** *(completed)* — core IP/route/interface types and all three platform backends shipped.
4. **Platform parity** *(completed)* — Linux, Windows, and BSD/macOS route, interface, DNS-read, neighbor-read, and address-read backends shipped.
5. **Advanced features** — monitoring, notifications, transactional configuration, and declarative networking.

## Contributing

Contributions are welcome once design and implementation work begins. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations, and [SECURITY.md](SECURITY.md) for reporting security issues.

## License

Net Lattice is licensed under the [Mozilla Public License 2.0](LICENSE) (`MPL-2.0`).
