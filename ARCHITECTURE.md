# Architecture

This document describes the planned workspace structure for Lattice and the
design principles behind it. It reflects intended direction, not current
state: see [CHANGELOG.md](CHANGELOG.md) and [README.md](README.md) for what
actually exists in the repository today. As of this writing, the repository
contains no crates and no implementation code.

## Guiding Principle

Lattice separates two concerns that are easy to tangle together in
cross-platform networking code:

1. **Model** — strongly typed representations of networking concepts (IP
   addresses, routes, interfaces, DNS configuration, ...) that carry no
   operating-system dependency whatsoever.
2. **Backend** — platform-specific code that reads and writes real operating
   system state (Linux Netlink, Windows IP Helper API, BSD/macOS route
   sockets) by producing and consuming the model's types.

Dependencies only ever point from backend toward model, never the reverse.
The model must never know that Linux, Windows, or macOS exist.

## Crate Boundary vs. Module Boundary

A concept gets its **own crate** only when there is a concrete reason to pull
it in isolation: independent reuse potential outside Lattice, a distinct
release cadence, or a real case for standalone publication to crates.io.
Everything else is a **module** inside a shared crate.

Applying this test:

- **IP addresses and networks** (`IPv4Address`, `IPv6Address`, `Network`,
  `Prefix`) are a real candidate for standalone use — a consumer might want
  typed IP parsing without any of the rest of Lattice. This justifies its own
  crate.
- **MAC addresses**, **routes**, **interfaces**, **neighbor entries (ARP/NDP)**,
  and **DNS configuration** have no independent reuse case: they are always
  used together, change together, and are meaningless without the rest of the
  networking model. These live as modules inside one model crate rather than
  as separate crates.

If a module later grows enough internal complexity to justify independent
versioning (for example, if DNS configuration grows a full resolver
implementation), it can be extracted into its own crate at that point. This
is a non-breaking workspace refactor, not an early commitment.

## Workspace Layout

```
lattice-core          Error, Result, ID types, shared traits
   │
   ├── lattice-ip        IPv4Address, IPv6Address, Network, Prefix
   │
   ├── lattice-model     modules: mac, route, interface, neighbor, dns
   │        (depends on: lattice-ip)
   │
   ├── lattice-platform  NetworkBackend trait, Capability, Event
   │        (depends on: lattice-model)
   │
   ├── lattice-linux     Netlink backend      (depends on: lattice-platform)
   ├── lattice-windows   IP Helper API backend (depends on: lattice-platform)
   ├── lattice-darwin    Route socket backend  (depends on: lattice-platform)
   │
   └── lattice           Public facade, default backend selection
```

### `lattice-core`

Foundational types with no networking semantics of their own: `Error`,
`Result<T>`, ID types (e.g. `InterfaceId`), and shared traits used across the
rest of the workspace. No OS dependency, no networking-specific types.

### `lattice-ip`

IP address and network primitives: `IPv4Address`, `IPv6Address`,
`IPv4Network`, `IPv6Network`, `PrefixLength`. Pure data and arithmetic, no
OS dependency. This crate should be buildable for any target, including
`wasm32`.

### `lattice-model`

The domain model of operating system networking state, organized as modules:

- `mac` — `MacAddress`
- `route` — `Route`, gateway, metric
- `interface` — `Interface` and interface kind (depends on `mac`)
- `neighbor` — ARP/NDP entries (depends on `lattice-ip` and `mac`)
- `dns` — DNS resolver configuration (depends on `lattice-ip`)

Modules within `lattice-model` may depend on each other and on `lattice-ip`,
but the crate as a whole has no OS dependency. `dns` intentionally does not
depend on `interface`; a per-interface DNS association is expressed with
`InterfaceId` from `lattice-core` rather than a direct module dependency, to
avoid coupling modules that should be free to evolve independently.

### `lattice-platform`

Defines the contract between the model and platform backends:

- `NetworkBackend` — the trait backends implement (list/add/remove routes,
  interfaces, neighbors, DNS configuration, event subscription).
- `Capability` — a way for backends to declare which optional features they
  support (e.g. VRF, namespaces, IPv6, monitoring), since platforms differ
  substantially in what they expose. Consumers query `supports(Capability)`
  rather than relying on features silently failing or panicking on
  unsupported platforms.
- `Event` — the event model for change notifications (interface state
  changes, route changes, ...), backed by RTNetlink multicast groups on
  Linux and `NotifyRouteChange2`-style APIs on Windows.

This crate depends on `lattice-model` but has no OS-specific code itself.

### Platform backends: `lattice-linux`, `lattice-windows`, `lattice-darwin`

Each backend implements `NetworkBackend` using native OS facilities:

- `lattice-linux` — Netlink (via an existing Netlink crate as a dependency,
  not a Lattice-owned wrapper crate).
- `lattice-windows` — IP Helper API via Windows bindings.
- `lattice-darwin` — BSD/macOS route sockets and related system APIs.

Platform-specific nuances that don't map to a single native API (for
example, DNS on Linux being served by systemd-resolved, NetworkManager, or
plain `resolv.conf` depending on the system) are resolved internally within
the backend crate — via capability detection — rather than by creating
separate crates per underlying mechanism.

Backends depend on `lattice-platform` and `lattice-model`. They export
nothing upward; nothing outside a backend crate depends on it directly
except `lattice` itself.

### `lattice`

The public-facing facade. Re-exports the types consumers need from
`lattice-model` and `lattice-ip`, selects a default backend based on
`cfg(target_os = "...")`, and exposes the top-level API (e.g.
`Lattice::connect()`). This is the only crate most consumers depend on
directly.

## Explicit Non-Goals of This Architecture

- **No crate is Linux-, Windows-, or macOS-specific except the backend
  crates themselves.** `lattice-core`, `lattice-ip`, `lattice-model`, and
  `lattice-platform` must remain free of `cfg(target_os = "...")` and OS
  bindings.
- **No command-line interface.** Consistent with the project's non-goals in
  [README.md](README.md), no `lattice-cli` crate is planned.
- **No premature crate creation.** Crates for future domains (VLAN, VRF,
  firewall, tunnels, declarative configuration, transactional apply/rollback)
  are described in the roadmap below but are not created until there is
  actual code to put in them.

## Incremental Delivery Plan

The full model above is a target, not a starting point. Crates and modules
are introduced only when there is real implementation work for them:

| Stage | Scope |
|-------|-------|
| 0.1 | `lattice-core`, `lattice-ip`, `lattice-model` (`route` module only), `lattice-platform`, `lattice-linux` (routes via Netlink), `lattice` |
| 0.2 | `lattice-windows` backend (routes) |
| 0.3 | `lattice-darwin` backend (routes) |
| 0.4 | `interface` module + CRUD across all backends |
| 0.5 | `dns` module |
| 0.6 | `neighbor` module (ARP/NDP) |
| 0.7+ | Capability-gated domains: VLAN, VRF, firewall integration, tunnels, event subscription, declarative configuration, transactional apply/rollback |

Each stage is expected to validate the architecture before the next is
started; earlier stages may inform adjustments to later ones.
