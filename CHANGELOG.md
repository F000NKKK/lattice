# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.3] - TBD

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
  OS instead of only ever compiling on Linux, plus a follow-up step per OS
  running each backend's `#[ignore]`d privileged round-trip test (`sudo` for
  `CAP_NET_ADMIN`/root on Linux/macOS; Windows runners are Administrator
  already). `dependabot.yml` gained a `github-actions` ecosystem entry now
  that a workflow exists for it to scan; both ecosystems' PRs run through
  this same CI.

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
- `net-lattice-backend-darwin`: `routes()` sent `RTM_GET` with no
  destination over the `PF_ROUTE` socket to try to dump the whole table —
  the kernel rejects that with `EINVAL` (`RTM_GET` looks up one specific
  destination's route; it isn't Netlink's dump-via-empty-request idiom).
  Caught by CI's first real run on a macOS runner. Replaced with
  `sysctl(CTL_NET, PF_ROUTE, 0, AF_UNSPEC, NET_RT_DUMP, 0)`, the standard
  BSD mechanism for reading the entire routing table at once.
- `net-lattice-backend-darwin`: `add_route` failed with `EINVAL` for any
  route with an interface index but no IP gateway (the common case —
  e.g. `route.with_interface_index(...)` alone). `RTM_ADD` requires an
  address in the `RTA_GATEWAY` slot to determine the outgoing path;
  `rtm_index` in the request header is not honored for `ADD` (the kernel
  only fills it in on the reply). Caught by CI's privileged round-trip
  test on a macOS runner. Fixed by supplying a link-layer (`AF_LINK`)
  `sockaddr_dl` gateway naming the interface when there's no IP gateway —
  the same shape `route add -interface` uses — without setting
  `RTF_GATEWAY` (that flag means "real next hop", not "no next hop, just
  this wire").
- `net-lattice-backend-darwin`: `add_route`/`remove_route` always wrapped
  the routing socket's errno in `Error::Platform`, even for errno values
  that map directly onto Net Lattice's error taxonomy per ARCHITECTURE.md's
  Error Model (`EEXIST` on `RTM_ADD` for a route that already exists,
  `ESRCH`/`ENOENT` on `RTM_DELETE` for no matching route, `EPERM`/`EACCES`
  for missing privilege) — callers had to pattern-match a raw Darwin errno
  instead of `Error::AlreadyExists`/`NotFound`/`PermissionDenied`. The
  privileged round-trip test also now cleans up any route left over from a
  prior interrupted run before adding, rather than surfacing that stale
  state as a fresh `AlreadyExists` failure.
- `net-lattice-backend-darwin`'s privileged round-trip test assumed `lo0`
  is always ifindex `1`; GitHub-hosted macOS runners carry enough virtual
  interfaces (Docker, VPN, `utun*`, ...) that this doesn't hold, so the
  test added a route on the wrong interface and never found it in
  `routes()` afterward. Looked up `lo0`'s real index via `InterfaceProvider`
  instead, same as the Linux/Windows equivalents of this test already did.
- `net-lattice-backend-darwin`: `add_route`/`remove_route` trusted a
  successful `send()` on the `PF_ROUTE` socket as proof the kernel actually
  performed the change. `send()` only confirms the message was accepted
  into the socket buffer — routing sockets echo the processed request back
  (with `rtm_errno` filled in) to every open routing socket on the system,
  and a caller is expected to read that reply to learn the real outcome.
  Diagnostics added for the previous two fixes proved this was live: the
  route was entirely absent afterward (not merely misfiled under the wrong
  interface or prefix), meaning `add_route` was reporting success for a
  request the kernel had silently rejected. Added `send_route_request`,
  which sends and then reads the socket until it sees the reply matching
  this request's `rtm_pid`/`rtm_seq` (filtering out other processes'
  traffic on the same shared socket), returning `Err` from a nonzero
  `rtm_errno` instead of assuming success.

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
