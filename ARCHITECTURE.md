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
   │        (depends on: lattice-core)
   │
   ├── lattice-model     modules: mac, route, interface, neighbor, dns, event
   │        (depends on: lattice-core, lattice-ip)
   │
   ├── lattice-platform  Generic provider traits, Capability
   │        (depends on: lattice-core — NOT lattice-model)
   │
   ├── lattice-backend-linux    Netlink backend
   ├── lattice-backend-windows  IP Helper API backend
   ├── lattice-backend-darwin   Route socket backend
   │        (each depends on: lattice-platform AND lattice-model —
   │         backends are where the generic contract and the concrete
   │         model finally meet)
   │
   └── lattice           Public facade, default backend selection
            (depends on: lattice-model, lattice-platform, lattice-backend-*)
```

`lattice-model` and `lattice-platform` are siblings under `lattice-core`,
not a chain. `lattice-platform` depends on nothing that describes what a
route or an interface actually is — it only knows that a backend produces
*something*, and defers what that something is to whoever implements or
consumes the trait. `lattice-model` in turn has no idea `lattice-platform`
exists. Neither can build the other into a `if linux { ... } else if
windows { ... }` situation, because neither has enough information about
the other's domain to do so. See the `lattice-platform` section below for
how this is expressed concretely.

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
- `event` — `Event`, the change-notification enum (`RouteAdded(Route)`,
  `InterfaceDown(Interface)`, ...). This lives here, not in
  `lattice-platform`, because an event's payload *is* domain data — it has
  no meaning without knowing what a `Route` or `Interface` is, which is
  exactly the knowledge `lattice-platform` is not allowed to have.

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

This is the crate that makes the model/backend separation real rather than
aspirational: **`lattice-platform` does not depend on `lattice-model`.**

Its provider traits describe the *shape* of a contract, not the *content*
of the model — they are generic over the domain type they operate on via
associated types, rather than naming `Route`/`Interface` from
`lattice-model` directly:

```rust
trait RouteProvider {
    type Route;

    fn routes(&self) -> Result<Vec<Self::Route>, Error>;
    fn add_route(&self, route: Self::Route) -> Result<(), Error>;
}

trait InterfaceProvider {
    type Interface;

    fn interfaces(&self) -> Result<Vec<Self::Interface>, Error>;
}
```

`lattice-platform` is satisfied by anything shaped like a route; it has no
way to know or care that the concrete type happens to come from
`lattice-model`. This is what "platform says *I need something
Route-shaped*; model says *I exist independently of platform*" means in
Rust terms — it is not achievable by wishing the dependency arrow away, it
requires the trait to stop naming the concrete type.

The provider traits, one per capability rather than one large trait, for
the same reason as before — a monolithic trait covering every domain would
force every backend to stub out methods for features it doesn't have:

- `RouteProvider` — list/add/remove routes.
- `InterfaceProvider` — list/configure interfaces.
- `NeighborProvider` — list ARP/NDP entries.
- `DnsProvider` — read/write DNS resolver configuration.
- `EventProvider` — subscribe to change notifications, generic over an
  associated `Event` type for the same reason as the others.

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
  actually available. `Capability` is a plain enum with no domain types in
  it, so it costs `lattice-platform` nothing to keep here.

This crate depends only on `lattice-core` (for `Error` and ID types). It
has no OS-specific code and, unlike the previous revision of this
document, no dependency on `lattice-model` either.

**Where the generic contract meets the concrete model.** Something has to
bind `Self::Route = lattice_model::route::Route` eventually, or the
associated types are never resolved to anything real. That binding
happens in the backend crates, which already depend on both
`lattice-platform` (for the traits) and `lattice-model` (for the concrete
types) — see below. `lattice-platform` itself never performs this binding
and never needs to.

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

Each backend binds every trait's associated type to the concrete
`lattice-model` type it produces:

```rust
impl RouteProvider for LinuxBackend {
    type Route = lattice_model::route::Route;

    fn routes(&self) -> Result<Vec<Self::Route>, Error> { /* netlink */ }
    fn add_route(&self, route: Self::Route) -> Result<(), Error> { /* netlink */ }
}
```

Backends are the only place in the workspace where `lattice-platform` and
`lattice-model` are both in scope at once.

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
- **`lattice-platform` never depends on `lattice-model`.** Its provider
  traits must stay generic over associated types rather than growing a
  direct dependency on concrete model types, even when it would be
  momentarily convenient (e.g. adding a new provider method whose most
  obvious signature names `lattice_model::route::Route` directly). If a
  provider trait cannot be expressed without naming a concrete model type,
  that is a signal to revisit the trait's shape, not to add the
  dependency.
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
| 0.7+ | Capability-gated domains: VLAN, VRF, firewall integration, tunnels; `event` module + `EventProvider`; `CurrentState`/`DesiredState`/`Diff`/`ApplyPlan` declarative configuration |

Each stage is expected to validate the architecture before the next is
started; earlier stages may inform adjustments to later ones.
