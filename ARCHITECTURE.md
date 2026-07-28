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
   ├── lattice-platform  Provider traits, Capability, Event
   │        (depends on: lattice-model)
   │
   ├── lattice-backend-linux    Netlink backend      (depends on: lattice-platform)
   ├── lattice-backend-windows  IP Helper API backend (depends on: lattice-platform)
   ├── lattice-backend-darwin   Route socket backend  (depends on: lattice-platform)
   │
   └── lattice           Public facade, default backend selection
```

### `lattice-core`

Foundational types with no networking semantics of their own: `Error`,
`Result<T>`, ID types (e.g. `InterfaceId`), and shared traits used across the
rest of the workspace. No OS dependency, no networking-specific types.

This crate is intentionally kept minimal and stays that way by construction:
anything that represents a networking concept (an address, a route, a
resolver setting) belongs in `lattice-ip` or `lattice-model`, never here.
`lattice-core` should never need a new module just because a new domain
(DNS, firewall, VLAN, ...) was added elsewhere in the workspace — if it does,
that is a sign something was misplaced.

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

**Model types must be designed for extension, not for the lowest common
denominator.** The same domain concept carries a different set of fields on
each platform — a route on Linux carries a routing table, protocol, scope,
and type in addition to destination/gateway/metric; Windows and BSD expose
a narrower set. Shrinking a model type down to only the fields every
platform happens to share now would make it impossible to add
platform-specific fields later without a breaking change. The concrete
field lists are an API design decision for the Stage 0.1 draft, not this
document, but whatever shape they take must leave room for
platform-specific extension (e.g. an open-ended properties/extension
container) from the outset.

### `lattice-platform`

Defines the contract between the model and platform backends, as a set of
narrow, capability-scoped provider traits rather than one large trait:

- `RouteProvider` — list/add/remove routes.
- `InterfaceProvider` — list/configure interfaces.
- `NeighborProvider` — list ARP/NDP entries.
- `DnsProvider` — read/write DNS resolver configuration.
- `EventProvider` — subscribe to change notifications (`Event` stream).

A backend implements only the provider traits it can actually support
natively — a monolithic `NetworkBackend` trait covering every domain would
force every backend to stub out methods for features it doesn't have, and
would grow without bound as new domains (VLAN, VRF, firewall, ...) are
added. Splitting by capability keeps each trait small:

```rust
trait RouteProvider {
    fn routes(&self) -> Result<RouteHandle>;
}

trait InterfaceProvider {
    fn interfaces(&self) -> Result<InterfaceHandle>;
}

trait EventProvider {
    fn events(&self) -> EventStream;
}
```

- `Capability` — distinct from provider traits, and deliberately not the
  same axis. Provider traits (`RouteProvider`, ...) describe API surfaces
  that are fixed at compile time: a backend either implements
  `DnsProvider` or it doesn't, and that's known when the backend crate is
  built. `Capability` describes *runtime*-dependent operating system
  features that cannot be expressed through Rust trait implementation alone
  — for example, a Linux backend always implements `RouteProvider`, but
  whether the running kernel has IPv6 or VRF support enabled is a fact
  about the current machine, not the crate. Consumers query
  `backend.capabilities().contains(Capability::VRF)` at runtime rather than
  relying on a method silently failing or panicking when a feature isn't
  actually available.
- `Event` — the event model for change notifications (interface state
  changes, route changes, ...), backed by RTNetlink multicast groups on
  Linux and `NotifyRouteChange2`-style APIs on Windows.

This crate depends on `lattice-model` but has no OS-specific code itself.

### Platform backends: `lattice-backend-linux`, `lattice-backend-windows`, `lattice-backend-darwin`

Each backend implements the subset of `lattice-platform` provider traits it
can support, using native OS facilities:

- `lattice-backend-linux` — Netlink (via an existing Netlink crate as a
  dependency, not a Lattice-owned wrapper crate).
- `lattice-backend-windows` — IP Helper API via Windows bindings.
- `lattice-backend-darwin` — BSD/macOS route sockets and related system
  APIs.

The `lattice-backend-*` naming (rather than bare `lattice-linux`, etc.)
makes each crate's role legible from its name alone when scanning the
workspace or `cargo search` results.

Platform-specific nuances that don't map to a single native API (for
example, DNS on Linux being served by systemd-resolved, NetworkManager, or
plain `resolv.conf` depending on the system) are resolved internally within
the backend crate — via capability detection — rather than by creating
separate crates per underlying mechanism. If a single backend crate ever
grows enough competing provider implementations for one domain to become
unwieldy (e.g. a Netlink-based `RouteProvider` and a NetworkManager-based
`DnsProvider` genuinely warranting independent release cycles), that domain
can be split into its own provider crate at that point — not before.

Backends depend on `lattice-platform` and `lattice-model`. They export
nothing upward; nothing outside a backend crate depends on it directly
except `lattice` itself.

### `lattice`

The public-facing facade. Re-exports the types consumers need from
`lattice-model` and `lattice-ip`, selects a default backend based on
`cfg(target_os = "...")`, and exposes the top-level API (e.g.
`Lattice::connect()`). This is the only crate most consumers depend on
directly.

## Error Model

Lattice must not leak `std::io::Error` or raw OS error codes
(`EPERM`/`ENODEV` on Linux, `ERROR_ACCESS_DENIED` on Windows) as its public
error type. Different backends fail for the same logical reason through
completely different codes, and a consumer writing cross-platform code
needs to match on *why* an operation failed, not on a platform-specific
integer.

`lattice-core::Error` is the single error type surfaced across the
workspace, expressed as platform-independent variants such as:

- `PermissionDenied`
- `NotFound`
- `AlreadyExists`
- `Unsupported` — the operation has no meaning on this backend at all (as
  opposed to a `Capability` being absent at runtime; see below).
- `InvalidState`
- `PlatformError { backend, code }` — an escape hatch that preserves the
  raw backend-specific error for diagnostics, without being the primary
  way consumers are expected to match on failures.

The exact variant list is an API design decision for the Stage 0.1 draft;
what this document fixes is that such a taxonomy exists and lives in
`lattice-core`, and that provider trait methods return `Result<T, Error>`
using it — never a raw OS error type.

## Privilege Model

Networking configuration is privileged on every target platform, and the
privilege boundary does not line up the same way across them:

- **Linux** — reading routes/interfaces is generally unprivileged; adding
  or removing them requires `CAP_NET_ADMIN`.
- **Windows** — reading is available to normal users; modifying typically
  requires Administrator.
- **BSD/macOS** — similar read/write asymmetry via route sockets.

This is not a hypothetical concern: it is the concrete scenario behind the
`Error::PermissionDenied` variant above, and it means read operations and
write operations should be expected to fail independently and for
different reasons in consumer code and in tests. This document does not
mandate a specific privilege-check API (e.g. a pre-flight
`backend.can_modify()`) — that is again an API design decision — but the
provider trait split (read-oriented listing vs. write-oriented add/remove
methods, already separate methods on the same trait) must not obscure the
fact that a caller can plausibly have one without the other.

## Async Model

`EventProvider` is inherently push-based on every platform (Netlink
multicast sockets on Linux, `NotifyRouteChange2`-style callbacks on
Windows, routing sockets on BSD/macOS), which means it cannot be
implemented as a plain blocking method the way `RouteProvider` or
`InterfaceProvider` can. Whether Lattice commits to an async runtime,
exposes a runtime-agnostic stream abstraction, or offers a blocking
callback-based API for `EventProvider` is an open decision that affects
the public API surface and dependency footprint (e.g. `futures`/`tokio`)
of every crate that touches events.

This decision must be made explicitly as part of the Stage 0.1 (or
whichever stage first implements `EventProvider`, per the delivery plan
below) API draft, before `EventProvider` is implemented for any backend —
not discovered ad hoc through the first backend that happens to implement
it. This document deliberately does not prescribe the answer.

## State Model: Imperative Now, Declarative Later

Lattice's initial API surface is imperative: `route.add()`, `route.delete()`,
mirroring what `RouteProvider`/`InterfaceProvider` naturally expose. This is
deliberate — it is the smallest useful surface and it maps directly onto
what native platform APIs provide.

However, declarative configuration is a stated long-term goal (see
README's Long-Term Goals), and it is a different way of using the same
provider traits, not a different backend contract. Retrofitting it later
would touch every provider if the concept isn't at least named now. The
architecture reserves room for it as:

- `CurrentState` — a snapshot assembled by reading providers (routes,
  interfaces, ...) for a given backend.
- `DesiredState` — the same shape, constructed by the consumer instead of
  read from the OS.
- `Diff` — the computed difference between `CurrentState` and
  `DesiredState`.
- `ApplyPlan` — an ordered sequence of provider calls (add/remove/modify)
  that would resolve a `Diff`, which can be inspected before being executed
  and rolled back if a step fails.

None of these types exist yet, and no crate is created for them now — they
belong to stage 0.7+ once enough of the imperative provider surface exists
to compute a meaningful diff against. They are named here so that
`CurrentState`/`DesiredState` are built from the same `lattice-model` types
as the imperative API from the start, rather than as a parallel model
introduced later.

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
| 0.1 | `lattice-core`, `lattice-ip`, `lattice-model` (`route` module only), `lattice-platform` (`RouteProvider`), `lattice-backend-linux` (routes via Netlink), `lattice` |
| 0.2 | `lattice-backend-windows` (`RouteProvider`) |
| 0.3 | `lattice-backend-darwin` (`RouteProvider`) |
| 0.4 | `interface` module + `InterfaceProvider` across all backends |
| 0.5 | `dns` module + `DnsProvider` |
| 0.6 | `neighbor` module + `NeighborProvider` (ARP/NDP) |
| 0.7+ | Capability-gated domains: VLAN, VRF, firewall integration, tunnels; `EventProvider`; `CurrentState`/`DesiredState`/`Diff`/`ApplyPlan` declarative configuration |

Each stage is expected to validate the architecture before the next is
started; earlier stages may inform adjustments to later ones.
