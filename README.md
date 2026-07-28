# Lattice

**Languages**

🇺🇸 **English** | 🇷🇺 [Русский](README.ru.md)

[![License: MPL 2.0](https://img.shields.io/badge/license-MPL--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org)
[![crates.io](https://img.shields.io/badge/crates.io-not%20yet%20published-lightgrey.svg)](https://crates.io)
[![docs.rs](https://img.shields.io/badge/docs.rs-not%20yet%20published-lightgrey.svg)](https://docs.rs)
[![MSRV](https://img.shields.io/badge/MSRV-1.93-lightgrey.svg)](Cargo.toml)

**Lattice** is a modern, cross-platform Rust library for configuring and inspecting operating system networking through a single, strongly typed API.

> **Status:** This repository is currently bootstrapped only. It contains no implementation, no crates, and no source code. Development has not yet begun.

## Overview

Operating systems expose networking configuration and state through wildly different, low-level, and often platform-specific interfaces: Linux Netlink, the Windows IP Helper API, BSD/macOS route sockets, and various vendor-specific mechanisms. Applications that need to inspect or configure networking — IP addresses, routes, interfaces, neighbors, and more — are typically forced to either shell out to external tools, parse text output, or write and maintain separate platform-specific integrations.

Lattice aims to unify these interfaces behind a single, strongly typed, idiomatic Rust API, so that consumers never need to deal with raw platform structures, shell commands, or ad hoc string parsing.

## Motivation

Cross-platform networking tooling in the Rust ecosystem is fragmented. Existing solutions are frequently platform-specific, incomplete, or built around shelling out to system utilities such as `ip`, `netsh`, or `route`. This is fragile, hard to test, and unsuitable for building robust, production-grade network management software.

Lattice is intended to fill this gap by providing a single, well-designed abstraction layer over native OS networking APIs.

## Philosophy

- **Strong typing over strings.** Consumers interact with typed Rust values — addresses, prefixes, routes, interfaces — never raw strings or shell commands.
- **Native APIs, not subprocesses.** Lattice talks directly to platform networking APIs (Netlink, IP Helper API, route sockets) rather than invoking external CLI tools.
- **Cross-platform by design.** A single API surface backed by platform-specific implementations, so applications do not need to special-case operating systems.
- **Correctness and safety first.** Network3ing configuration is sensitive; the library should make incorrect states difficult to represent.
- **Incremental, well-considered growth.** Features are added deliberately, with attention to API design and long-term maintainability, rather than rushed to cover every possible use case.

## Long-Term Goals

Lattice intends to eventually provide support for:

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

- Lattice is not a replacement for full network management daemons (e.g. NetworkManager, systemd-networkd).
- Lattice does not aim to provide a command-line interface or GUI as part of the core library.
- Lattice does not aim to parse or wrap the output of external CLI tools as a long-term strategy.
- Lattice does not aim to support every conceivable network protocol or vendor extension from day one.

## Current Status

This repository currently contains **only bootstrap infrastructure**:

- A Cargo workspace with no members
- Repository metadata and community health files
- GitHub configuration (issue templates, pull request template, Dependabot)

No crates, source code, examples, tests, or benchmarks exist yet. No API design or architectural decisions have been made. This is intentional: the repository is being initialized ahead of design and implementation work.

## Roadmap

1. **Bootstrap** *(current stage)* — repository infrastructure, licensing, community health files, and tooling configuration.
2. **Design** — define the crate layout, core abstractions, and platform abstraction strategy.
3. **Foundations** — implement core types (addresses, prefixes, interfaces) and the first platform backend.
4. **Platform parity** — extend support across Linux, Windows, and BSD/macOS.
5. **Advanced features** — monitoring, notifications, transactional configuration, and declarative networking.

## Contributing

Contributions are welcome once design and implementation work begins. See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines, [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community expectations, and [SECURITY.md](SECURITY.md) for reporting security issues.

## License

Lattice is licensed under the [Mozilla Public License 2.0](LICENSE) (`MPL-2.0`).
