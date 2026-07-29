# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - TBD

### Added

- `net-lattice-model`: the `mac` module (`MacAddress`) and the `interface`
  module (`Interface`, `InterfaceId`, `InterfaceKind`, `AdminState`,
  `OperationalState`).
- `net-lattice-platform`: `InterfaceProvider`, with no dependency on
  `net-lattice-model`.
- `net-lattice-backend-linux`: `InterfaceProvider` implementation via
  Netlink (`RTM_GETLINK`), covered by a real, unprivileged `interfaces()`
  test asserting the `lo` interface is present and classified as
  `Loopback`.
- `net-lattice-backend-windows`: `InterfaceProvider` implementation via the
  Windows IP Helper API (`GetIfTable2`).
- `net-lattice-backend-darwin`: `InterfaceProvider` implementation via
  `getifaddrs`, reading `AF_LINK` entries for name, index, hardware type,
  and MAC address, plus MTU via `ioctl(SIOCGIFMTU)`.
- `net-lattice` facade: `Lattice::interfaces()`, and `LatticeBackend` now
  additionally requires `InterfaceProvider<Interface = Interface>`.
- CI: `.github/workflows/ci.yml` runs fmt/clippy/test/doc on native Linux,
  Windows, and macOS GitHub-hosted runners (not cross-compiled), so each
  platform's backend crate is actually built and clippy-checked on its own
  OS instead of only ever compiling on Linux. `dependabot.yml` gained a
  `github-actions` ecosystem entry now that a workflow exists for it to
  scan; both ecosystems' PRs run through this same CI.

This is Stage 0.4 of [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental
Delivery Plan: listing network interfaces on Linux, Windows, and
BSD/macOS.

### Fixed

- `net-lattice-backend-darwin`: route parsing (`message_to_route`) always
  reported destinations as `/32`/`/128` regardless of the actual route,
  since `RTA_NETMASK` was never parsed. Non-host routes (subnets, the
  default route) now get their real prefix length from the netmask.
- `net-lattice-backend-windows`: `RouteProvider` used field/type names that
  don't exist on the real `windows` crate bindings (`MIB_IPADDRESS_STRING`,
  `Metric1`, raw `u16`/`u32` casts of `WIN32_ERROR`/`ADDRESS_FAMILY`) and
  never actually compiled for `target_os = "windows"`.

## [0.3.0] - TBD

### Added

- `net-lattice-backend-darwin`: `RouteProvider` implementation via BSD/macOS
  route sockets (`RTM_GET`/`RTM_ADD`/`RTM_DELETE`), gated to
  `target_os = "macos"`. Covered by a real, unprivileged `routes()` test
  and a root-gated add/remove round-trip test (`#[ignore]`, run manually
  with elevated privileges).
- macOS platform support in the `net-lattice` facade (`Lattice::connect()`
  on `cfg(target_os = "macos")`), wired to `DarwinBackend`.

This is Stage 0.3 of [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental
Delivery Plan: listing, adding, and removing IPv4/IPv6 routes on BSD/macOS.


## [0.2.0] - TBD

### Added

- `net-lattice-backend-windows`: `RouteProvider` implementation via the
  Windows IP Helper API (`GetIpForwardTable2`, `CreateIpForwardEntry2`,
  `DeleteIpForwardEntry2`), gated to `target_os = "windows"`.
- Windows platform support in the `net-lattice` facade (`Lattice::connect()`
  on `cfg(target_os = "windows")`), wired to `WindowsBackend`.

This is Stage 0.2 of [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental
Delivery Plan: listing, adding, and removing IPv4/IPv6 routes on Windows.


## [0.1.0] - 2026-07-28

### Added


- Repository bootstrap: workspace `Cargo.toml`, licensing, community health
  files, and GitHub configuration.
- `net-lattice-core`: `Error`, `PlatformErrorCode`, and `Id<T>`.
- `net-lattice-ip`: `Ipv4Address`/`Ipv6Address`, `Ipv4Network`/`Ipv6Network`,
  `Ipv4PrefixLength`/`Ipv6PrefixLength`.
- `net-lattice-model`: the `route` module (`Route`, `RouteId`, `IpAddress`,
  `Network`), including `Route::interface_index` for specifying the
  outgoing interface by raw ifindex.
- `net-lattice-platform`: `RouteProvider` and `Capability`, with no
  dependency on `net-lattice-model`.
- `net-lattice-backend-linux`: `RouteProvider` implementation via Netlink
  (`rtnetlink`), gated to `target_os = "linux"`. Covered by a real,
  unprivileged `routes()` test and a `CAP_NET_ADMIN`-gated add/remove
  round-trip test (`#[ignore]`, run manually with elevated privileges).
- `net-lattice` facade: `LatticeBackend`, `Lattice<B>`, and
  `Lattice::connect()` (Linux default), plus a `list_routes` example.
- `scripts/release.sh` and `scripts/gh_release.sh` for versioning,
  publishing, and tagging workspace crates.

This is Stage 0.1 of [ARCHITECTURE.md](ARCHITECTURE.md)'s Incremental
Delivery Plan: listing, adding, and removing IPv4/IPv6 routes on Linux.
