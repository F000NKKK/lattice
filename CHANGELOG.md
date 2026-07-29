# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
