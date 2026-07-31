# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.12.3] - 2026-07-31

### Documentation

- Document `EventReceiver` bounded delivery, cancellation, timeout,
  disconnection, constructor, subscription-guard, and iterator semantics.
- Clarify backend-facing native Tokio watcher integration through
  `TokioEventProvider`, including associated types and receiver ownership.
- Update `EventStream` documentation to cover both native async delivery and
  synchronous receiver adaptation; add watcher-oriented rustdoc examples.
- Clarify the observed resolver-view semantics of `DnsConfig` and the
  portable-runtime contract of `Capability`.

### Tests

- Add lifecycle and regression coverage for subscription-guard replacement,
  receiver drop, iterator error termination, and zero-capacity rejection.

## [0.12.2] - 2026-07-31

### Added

- Stage 0.12 watcher API stabilization: `EventFilter` object selectors for
  routes, interfaces, neighbors, and interface addresses; filters are applied
  before ordinary events enter backend queues.
- Stage 0.11 `Lattice::watch_async(filter)` now shares the object/domain
  filter semantics of `Lattice::watch_filtered(filter)` without a duplicate
  async watcher entry point.

### Changed

- Facade watcher entry points now validate `Capability::MONITORING` before
  opening a native subscription and return `Error::Unsupported` when absent.

## [0.11.0] - 2026-07-31

### Added

- Optional `net-lattice` `async` feature and `Lattice::watch_async(filter)`.
  The feature re-exports one runtime-agnostic `net-lattice-async::EventStream`
  without adding Tokio code to the default facade.
- Native async event delivery in every backend: Linux polls Netlink through
  its existing Tokio runtime, Windows IP Helper callbacks feed a Tokio
  transport, and the macOS PF_ROUTE reader feeds that transport directly.
  All three use bounded delivery with the existing resynchronization semantics.
- `net-lattice-async` 0.1.0: the single `futures::Stream` type used by the
  facade. It also retains an explicit worker-thread bridge for callers that
  adapt an arbitrary synchronous `EventReceiver` themselves.

## [0.10.0] - 2026-07-31

### Added

- Bounded event delivery (256 entries), `Event::ResyncRequired` overflow
  signalling, `EventFilter`, and `Lattice::watch_filtered()` across Linux,
  Windows, and macOS native watchers.

## [0.9.1] - 2026-07-31

### Fixed

- `net-lattice-backend-darwin`: write IPv4 octets to `sockaddr_in` in the
  correct network byte order for native address ioctls. The previous encoding
  could make a successful `SIOCAIFADDR` assignment unreadable through the
  requested `InterfaceAddress`, causing `add_address()` to return
  `Error::InvalidState` on macOS. A regression test now verifies the exact
  `sockaddr_in` round trip.

## [0.9.0] - 2026-07-31

### Added

- `net-lattice-model`: `NewInterfaceAddress`, a separate, typed intent for
  assigning an interface address. It accepts an interface ID, address/prefix,
  and optional IPv4 broadcast; the caller never constructs an observed
  `InterfaceAddressId`.
- `net-lattice-platform`: `AddressMutator`, separate from read-only
  `AddressProvider`, with `add_address` returning the canonical observed
  address and `remove_address` accepting that observed record.
- `net-lattice-backend-linux`: native IPv4/IPv6 address assignment and
  removal through Netlink.
- `net-lattice-backend-windows`: native IPv4/IPv6 address assignment and
  removal through `CreateUnicastIpAddressEntry` and
  `DeleteUnicastIpAddressEntry`. Windows rejects an explicit IPv4 broadcast,
  because IP Helper derives it from the prefix instead of accepting an
  override.
- `net-lattice-backend-darwin`: native IPv4/IPv6 address assignment and
  removal through BSD address ioctls; no `ifconfig` subprocess is used.
- `net-lattice` facade: `Lattice::add_address()` and
  `Lattice::remove_address()`, with model convergence enforced for
  `AddressMutator` as well as `AddressProvider`.

### Testing

- Privileged end-to-end add/read/remove address tests for Linux, Windows, and
  BSD/macOS, using a dedicated TEST-NET-1 address on the loopback interface.

## [0.8.0] - 2026-07-30

### Added

- `net-lattice-platform`: `CapabilityProvider`, reporting the runtime-
  dependent `Capability` flags (previously only a bare bitflags type with
  no way for a caller to actually query it) the connected backend
  currently has available.
- `net-lattice-backend-linux`/`-windows`/`-darwin`: `CapabilityProvider`
  implementation, reporting `Capability::IPV6` (every provider these
  backends implement already handles both address families).
  `VRF`/`NAMESPACES`/`MONITORING` are left unset — not implemented yet.
- `net-lattice` facade: `Lattice::capabilities()` and
  `Lattice::supports(capability)`, and `LatticeBackend` now additionally
  requires `CapabilityProvider`.
- `net-lattice-model`: `event` module with signal-shaped `Event` and
  `ChangeKind` values for routes, interfaces, neighbors, and interface
  addresses.
- `net-lattice-platform`: synchronous `EventProvider` and `EventReceiver`.
  The receiver provides `recv`, `try_recv`, `recv_timeout`, and `Iterator`,
  without adding an async-runtime dependency to the core API.
- `net-lattice-backend-linux`: monitoring through an independent Netlink
  multicast socket for link, route, neighbor, and address notifications.
- `net-lattice-backend-windows`: monitoring through IP Helper route,
  interface, and unicast-address change registrations.
- `net-lattice-backend-darwin`: monitoring through an independent PF_ROUTE
  socket for route, interface, address, and neighbor notifications.
- All backends advertise `Capability::MONITORING` and retain native watcher
  cancellation state in the returned `EventReceiver`.
- `net-lattice` facade: `Lattice::watch()` plus event-related re-exports.

### Changed

- `net-lattice-core`: `Error::Disconnected` distinguishes a stopped watcher
  from an empty event queue or a timeout.

## [0.7.0] - 2026-07-30

### Added

- `net-lattice-model`: the `ifaddr` module (`InterfaceAddress`,
  `InterfaceAddressId` — an IP address assigned to an interface, plus its
  prefix length and, for IPv4, its broadcast address). Named `ifaddr`
  rather than `address` to avoid colliding with the existing
  `IpAddress`/`Network` primitives.
- `net-lattice-platform`: `AddressProvider`, with no dependency on
  `net-lattice-model`.
- `net-lattice-backend-linux`: `AddressProvider` implementation via
  Netlink's `RTM_GETADDR` (`rtnetlink`'s `address()` handle).
- `net-lattice-backend-darwin`: `AddressProvider` implementation via
  `getifaddrs`'s `AF_INET`/`AF_INET6` entries, reusing the interface
  backend's existing traversal.
- `net-lattice-backend-windows`: `AddressProvider` implementation via
  `GetUnicastIpAddressTable`.
- `net-lattice` facade: `Lattice::addresses()`, and `LatticeBackend` now
  additionally requires `AddressProvider<InterfaceAddress = InterfaceAddress>`.

## [0.6.0] - 2026-07-30

### Added

- `net-lattice-model`: the `neighbor` module (`NeighborEntry`, `NeighborId`,
  `NeighborState` — mirroring Linux's `NUD_*`/BSD's route-socket flags/
  Windows's `NL_NEIGHBOR_STATE`).
- `net-lattice-platform`: `NeighborProvider`, with no dependency on
  `net-lattice-model`.
- `net-lattice-backend-linux`: `NeighborProvider` implementation via
  Netlink's `RTM_GETNEIGH` (`rtnetlink`'s `neighbours()` handle).
- `net-lattice-backend-darwin`: `NeighborProvider` implementation via
  `sysctl(NET_RT_FLAGS, RTF_LLINFO)` over `AF_INET`/`AF_INET6` — the same
  mechanism `arp -a`/`ndp -an` use — reusing the `rt_msghdr` parsing
  already in place for route dumps.
- `net-lattice-backend-windows`: `NeighborProvider` implementation via
  `GetIpNetTable2`.
- `net-lattice` facade: `Lattice::neighbors()`, and `LatticeBackend` now
  additionally requires `NeighborProvider<NeighborEntry = NeighborEntry>`.

## [0.5.0] - 2026-07-30

### Added

- `net-lattice-model`: the `dns` module (`DnsConfig`: nameservers and
  search domains).
- `net-lattice-platform`: `DnsProvider`, with no dependency on
  `net-lattice-model`.
- `net-lattice-backend-linux` and `net-lattice-backend-darwin`:
  `DnsProvider` implementation via parsing `/etc/resolv.conf`
  (`nameserver`/`search`/`domain` directives — identical format on both
  platforms, so the parser is shared verbatim between the two crates).
- `net-lattice-backend-windows`: `DnsProvider` implementation via
  `GetAdaptersAddresses`, aggregating each adapter's DNS servers and
  suffix, deduplicated across the machine.
- `net-lattice` facade: `Lattice::dns_config()`, and `LatticeBackend` now
  additionally requires `DnsProvider<DnsConfig = DnsConfig>`.

### Fixed

- `scripts/release.sh`: the root `Cargo.toml` `[workspace.dependencies]`
  version reference for a crate could silently fail to update on a
  minor/major bump. The check required the crate's *current* version to
  match, character-for-character, whatever was already pinned in root
  `Cargo.toml` — but patch bumps intentionally never touch root (they're
  semver-compatible via caret), so after enough patch bumps accumulated,
  root's pinned version drifted behind the crate's real version, and the
  match silently failed on the next minor/major bump, leaving root stale
  (observed live: `net-lattice-backend-darwin` bumped 0.2.11 → 0.3.0 while
  root Cargo.toml kept referencing 0.2.10). Fixed by replacing whatever
  version is present for that dependency, unconditionally, instead of
  requiring an exact match against the old version.
- `scripts/release.sh`: the crates.io "is this version already published?"
  guard (which exists to avoid skipping past a version that was bumped in
  git but never actually published) only ran when the *current* version
  was "round" relative to the requested bump (e.g. patch `0` before a
  `--minor`). A non-round current version (e.g. `0.2.11`) skipped the
  check entirely and bumped straight past it even if unpublished. Fixed:
  the publication check now runs before every `--minor`/`--major` bump
  regardless of whether the current version is round; `--patch` still
  skips it, since a patch bump is always semver-compatible with what it
  replaces.

## [0.4.10] - 2026-07-30

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
- `net-lattice-backend-darwin`: the `AF_LINK` `sockaddr_dl` gateway used
  for interface-only routes (no IP next hop) declared `sdl_len` as
  `sizeof(struct sockaddr_dl)` (20 bytes, including a 12-byte `sdl_data`
  array left entirely unused). Compared against `golang.org/x/net/route`
  (the reference BSD routing-socket implementation)'s `LinkAddr.marshal`,
  the on-wire `sdl_len` for a name-less, address-less link address must be
  just the significant header (8 bytes) — not the padded struct size. The
  kernel accepted the malformed request (`rtm_errno == 0`) but never
  actually created the route, which is why `routes()` found nothing under
  that destination at all afterward, on any interface. `sdl_len` and the
  bytes actually written are now both 8. This alone didn't fix the round
  trip, though — see the next entry.
- `net-lattice-backend-darwin`: the `AF_LINK` gateway for interface-only
  routes carried only `sdl_index`, an empty name (`sdl_nlen = 0`). Checked
  against `route.tproj/route.c` (Apple's own `route(8)` source, via
  `apple-oss-distributions/network_cmds`) — `route add -interface`
  resolves the interface's real `sockaddr_dl` via `getifaddrs` and copies
  it *whole*, name included, into the gateway slot. An index-only
  `sockaddr_dl` is accepted by the kernel (`rtm_errno == 0`, matching what
  CI observed) but doesn't resolve to a usable interface reference, so no
  route is actually created. `push_link_gateway` now looks up the
  interface's name via `if_indextoname` and includes it (`sdl_nlen`/
  `sdl_data`), matching what a real, working `sockaddr_dl` for that
  interface looks like. Still didn't fix the round trip — see the next
  entry, which turned out to be the actual root cause of every `add_route`
  failure so far in this stage.
- `net-lattice-backend-darwin`: `build_add_message`/`build_delete_message`
  wrote the destination's sockaddrs to the message body in `DST, NETMASK,
  GATEWAY` order. BSD routing-socket messages require sockaddrs in
  ascending `RTAX_*` index order (`DST`=0, `GATEWAY`=1, `NETMASK`=2, ...) —
  confirmed against `golang.org/x/net/route`'s `marshalAddrs`, which
  iterates addresses strictly by that index when building the wire
  format. Every previous fix in this stage (netmask parsing, the link
  gateway's shape, its `sdl_len`, its name) was individually correct but
  moot: with gateway and netmask swapped, the kernel was reading the
  netmask sockaddr as the gateway and the gateway/link-layer sockaddr as
  the netmask, on every `RTM_ADD`/`RTM_DELETE` this backend ever sent.
  Reordered to `DST, GATEWAY, NETMASK` in both functions.
- `net-lattice-backend-darwin`: `push_link_gateway` returned the
  unrounded `sdl_len` (e.g. 11 for an 8-byte header plus a 3-byte
  interface name like `lo0`) as the buffer space it consumed, instead of
  that value rounded up to the routing socket's 4-byte alignment (12).
  Every subsequent sockaddr in the message (`NETMASK`, in this backend's
  case) then landed at a misaligned offset, corrupting how the kernel
  parsed everything after the link-layer gateway. Confirmed by hex-dumping
  the exact bytes sent and the kernel's own reply in CI: the reply's
  `rtm_errno` was `17` (`EEXIST`) — a real error the round-trip test's
  `if matches!(..., Err(PermissionDenied) | Err(Platform(_)))` guard
  didn't catch, since `EEXIST` correctly maps to `Error::AlreadyExists`,
  silently letting a genuinely failed `add_route` continue as if nothing
  had gone wrong. Fixed `push_link_gateway` to return the rounded-up
  length, and hardened the test to fail on *any* `add_route` error rather
  than only those two variants.

## [0.3.0] -  2026-07-29

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


## [0.2.0] - 2026-07-29

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
