# Architecture

**Languages**

🇺🇸 **English** | 🇷🇺 [Русский](ARCHITECTURE.ru.md)

This document describes the planned workspace structure for Net Lattice and the
design principles behind it. It reflects intended direction, not current
state: see [CHANGELOG.md](CHANGELOG.md) and [README.md](README.md) for what
actually exists in the repository today. As of this writing, Stage 0.14 of
the Incremental Delivery Plan below has landed: `net-lattice-core`,
`net-lattice-ip`, `net-lattice-model`'s `route`, `interface`, `dns`,
`neighbor`, `ifaddr`, and `mutation` modules, `net-lattice-platform`'s `RouteProvider`,
`InterfaceProvider`, `DnsProvider`, `DnsMutator`, `NeighborProvider`, and
`AddressProvider`, `AddressMutator`, `CapabilityProvider`, synchronous `EventProvider`,
feature-gated `TokioEventProvider`, and object/domain `EventFilter` selectors,
route/interface-address/DNS/neighbor support, native route/address/DNS
mutation, inspectable mutation plans, and native event monitoring in
`net-lattice-backend-linux`, `net-lattice-backend-windows`, and
`net-lattice-backend-darwin`, the `net-lattice-async` event stream crate, and
the feature-gated async facade — everything past that stage is still a target,
not current state.

## Guiding Principle

Net Lattice separates two concerns that are easy to tangle together in
cross-platform networking code:

1. **Model** — strongly typed representations of networking concepts (IP
   addresses, routes, interfaces, DNS configuration, ...) that carry no
   operating-system dependency whatsoever.
2. **Backend** — platform-specific code that reads and writes real operating
   system state (Linux Netlink, Windows IP Helper API, macOS BSD route
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
  typed IP parsing without any of the rest of Net Lattice. This justifies its own
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
net-lattice-core          Error, Result, ID types, shared traits
   │
   ├── net-lattice-ip        IPv4Address, IPv6Address, Network, Prefix
   │        (depends on: net-lattice-core)
   │
   ├── net-lattice-model     modules: mac, route, interface, neighbor, dns, event, mutation
   │        (depends on: net-lattice-core, net-lattice-ip)
   │
   ├── net-lattice-platform  Generic provider traits, Capability
   │        (depends on: net-lattice-core — NOT net-lattice-model)
   │
   ├── net-lattice-backend-linux    Netlink backend
   ├── net-lattice-backend-windows  IP Helper API backend
   ├── net-lattice-backend-darwin   Route socket backend
   │        (each depends on: net-lattice-platform AND net-lattice-model —
   │         backends are where the generic contract and the concrete
   │         model finally meet)
   │
   └── net-lattice           Public facade, default backend selection
            (depends on: net-lattice-model, net-lattice-platform, net-lattice-backend-*)
```

`net-lattice-model` and `net-lattice-platform` are siblings under `net-lattice-core`,
not a chain. `net-lattice-platform` depends on nothing that describes what a
route or an interface actually is — it only knows that a backend produces
*something*, and defers what that something is to whoever implements or
consumes the trait. `net-lattice-model` in turn has no idea `net-lattice-platform`
exists. Neither can build the other into a `if linux { ... } else if
windows { ... }` situation, because neither has enough information about
the other's domain to do so. See the `net-lattice-platform` section below for
how this is expressed concretely.

### `net-lattice-core`

Foundational types with no networking semantics of their own: `Error`,
`Result<T>`, ID types, and shared traits used across the rest of the
workspace. No OS dependency, no networking-specific types.

**ID types are a single generic type, not one struct per domain object.**
Rather than defining `RouteId`, `InterfaceId`, `NeighborId`, ... as
independent structs, `net-lattice-core` defines one phantom-typed `Id<T>` and
each domain gets a type alias (`type InterfaceId = Id<Interface>;`). This
costs nothing extra to define and makes a whole class of mistake a compile
error instead of a runtime bug: passing a `RouteId` where an `InterfaceId`
is expected fails to compile, rather than silently looking up the wrong
object. The exact shape of `Id<T>` is a Stage 0.1 API detail; what this
document fixes is that IDs are one generic mechanism, not N hand-written
lookalike structs.

`Id<T>` must have a stable, `T`-independent serialized representation
(e.g. it serializes as its underlying value, not as a struct carrying a
phantom marker) from the moment serialization is added. IDs are exactly
the kind of type that ends up embedded in `RouteConfig`/`DesiredState`
persisted to disk or sent over a wire (see State Model below); changing
their wire format after the fact would be a breaking change to every
stored or transmitted config.

This crate is intentionally kept minimal and stays that way by construction:
anything that represents a networking concept (an address, a route, a
resolver setting) belongs in `net-lattice-ip` or `net-lattice-model`, never here.
`net-lattice-core` should never need a new module just because a new domain
(DNS, firewall, VLAN, ...) was added elsewhere in the workspace — if it does,
that is a sign something was misplaced.

### `net-lattice-ip`

IP address and network primitives: `IPv4Address`, `IPv6Address`,
`IPv4Network`, `IPv6Network`, `PrefixLength`. Pure data and arithmetic, no
OS dependency. This crate should be buildable for any target, including
`wasm32`.

### `net-lattice-model`

The domain model of operating system networking state, organized as modules:

- `mac` — `MacAddress`
- `route` — `Route`, gateway, metric
- `interface` — `Interface` and interface kind (depends on `mac`)
- `neighbor` — ARP/NDP entries (depends on `net-lattice-ip` and `mac`)
- `dns` — DNS resolver configuration (depends on `net-lattice-ip`)
- `ifaddr` — IP addresses assigned to interfaces, including observed
  `InterfaceAddress` records and `NewInterfaceAddress` assignment intent
  (depends on `net-lattice-ip`;
  named `ifaddr` rather than `address` to avoid colliding with
  `net-lattice-ip`/`net-lattice-model`'s own `IpAddress`/`Network`
  primitives — this is the distinct concept of an address *bound to an
  interface*, not another address representation)
- `event` — `Event`, the change-notification enum. This lives here, not in
  `net-lattice-platform`, because an event refers to domain data — it has no
  meaning without knowing what a route or an interface is, which is
  exactly the knowledge `net-lattice-platform` is not allowed to have.

  **Events are signals, not snapshots.** An event should carry an ID and a
  kind of change (`Added` / `Removed` / `Changed`), not a clone of the full
  domain object:

  ```rust
  pub enum Event {
      Route { id: RouteId, kind: ChangeKind },
      Interface { id: InterfaceId, kind: ChangeKind },
  }
  ```

  rather than `Event::RouteAdded(Route)`. Two reasons: native change
  notifications frequently don't hand over the full object in the first
  place (an `RTM_NEWROUTE` message or a Windows route-change callback can
  carry only what changed, not a complete record), so an `Event` that
  demands a full `Route` would force backends to reconstruct one that
  wasn't actually delivered; and cloning a full domain object on every
  change is wasted work when most consumers only want to know *that*
  something changed before deciding whether to re-query it. A consumer
  that needs the current value re-reads it through the relevant provider
  (`backend.routes()`, keyed by the ID from the event).

  `ChangeKind::Changed` should eventually carry which fields changed
  (e.g. `Changed { fields: RouteFieldMask }`), not be a bare marker.
  Without it, a consumer that only cares about gateway changes still has
  to re-fetch and diff the whole object on every unrelated metric update,
  which defeats much of the point of a signal-shaped event. The exact
  field-mask representation is a Stage 0.8 (or whenever `EventProvider`
  ships) API detail; what this document fixes is that `Changed` is
  expected to carry this information eventually, so the enum shape isn't
  designed to preclude it.

Modules within `net-lattice-model` may depend on each other and on `net-lattice-ip`,
but the crate as a whole has no OS dependency. `dns` intentionally does not
depend on `interface`; a per-interface DNS association is expressed with
`InterfaceId` from `net-lattice-core` rather than a direct module dependency, to
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

### `net-lattice-platform`

This is the crate that makes the model/backend separation real rather than
aspirational: **`net-lattice-platform` does not depend on `net-lattice-model`.**

Its provider traits describe the *shape* of a contract, not the *content*
of the model — they are generic over the domain type they operate on via
associated types, rather than naming `Route`/`Interface` from
`net-lattice-model` directly:

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

`net-lattice-platform` is satisfied by anything shaped like a route; it has no
way to know or care that the concrete type happens to come from
`net-lattice-model`. This is what "platform says *I need something
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
- `AddressProvider` — list IP addresses assigned to interfaces.
- `AddressMutator` — assign and remove IP addresses. Its input is distinct
  from its observed output: `NewInterfaceAddress` has an interface ID,
  address/prefix, and optional IPv4 broadcast, while `InterfaceAddress` has
  a backend-derived ID and any attributes reported by the OS.
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
  it, so it costs `net-lattice-platform` nothing to keep here. Because
  consumers routinely need to check for combinations of capabilities
  (`caps.contains(Capability::IPV6 | Capability::VRF)`), it should be
  represented as a bitflags-style value rather than as a `Vec<Capability>`
  or `HashSet<Capability>` — combination and containment checks are then
  cheap bitwise operations instead of collection scans. The exact
  representation (`bitflags!`-generated type vs. a hand-rolled one) is a
  Stage 0.1 detail; what this document fixes is that `Capability` is a
  flag set, not a list.

This crate depends only on `net-lattice-core` (for `Error` and ID types). It
has no OS-specific code and, unlike the previous revision of this
document, no dependency on `net-lattice-model` either.

**Where the generic contract meets the concrete model.** Something has to
bind `Self::Route = net_lattice_model::route::Route` eventually, or the
associated types are never resolved to anything real. That binding
happens in the backend crates, which already depend on both
`net-lattice-platform` (for the traits) and `net-lattice-model` (for the concrete
types) — see below. `net-lattice-platform` itself never performs this binding
and never needs to.

### Platform backends: `net-lattice-backend-linux`, `net-lattice-backend-windows`, `net-lattice-backend-darwin`

Each backend implements the subset of `net-lattice-platform` provider traits it
can support, using native OS facilities:

- `net-lattice-backend-linux` — Netlink (via an existing Netlink crate as a
  dependency, not a Net Lattice-owned wrapper crate).
- `net-lattice-backend-windows` — IP Helper API via Windows bindings.
- `net-lattice-backend-darwin` — macOS BSD route sockets and related system
  APIs.

The `net-lattice-backend-*` naming (rather than bare `net-lattice-linux`, etc.)
makes each crate's role legible from its name alone when scanning the
workspace or `cargo search` results, and leaves room for names like
`net-lattice-backend-linux-networkmanager` alongside
`net-lattice-backend-linux-netlink` if a given OS ever needs more than one
competing backend crate.

`net-lattice` selects a backend crate for the current target by default via
`cfg(target_os = "...")`, but each backend is additionally gated behind a
same-named Cargo feature (`linux`, `windows`, `darwin`). This is not about
runtime backend switching (see the object safety note above — that stays
a compile-time choice) but about being able to depend on `net-lattice-backend-linux`
specifically — for example to run its unit tests, or to cross-check its
behavior — without requiring a full build for every other platform's
backend on a machine that can't target them.

Each backend binds every trait's associated type to the concrete
`net-lattice-model` type it produces:

```rust
impl RouteProvider for LinuxBackend {
    type Route = net_lattice_model::route::Route;

    fn routes(&self) -> Result<Vec<Self::Route>, Error> { /* netlink */ }
    fn add_route(&self, route: Self::Route) -> Result<(), Error> { /* netlink */ }
}
```

Backends are the only place in the workspace where `net-lattice-platform` and
`net-lattice-model` are both in scope at once.

Platform-specific nuances that don't map to a single native API (for
example, DNS on Linux being served by systemd-resolved, NetworkManager, or
plain `resolv.conf` depending on the system) are resolved internally within
the backend crate — via capability detection — rather than by creating
separate crates per underlying mechanism. If a single backend crate ever
grows enough competing provider implementations for one domain to become
unwieldy (e.g. a Netlink-based `RouteProvider` and a NetworkManager-based
`DnsProvider` genuinely warranting independent release cycles), that domain
can be split into its own provider crate at that point — not before.

Backends depend on `net-lattice-platform` and `net-lattice-model`. They export
nothing upward; nothing outside a backend crate depends on it directly
except `net-lattice` itself.

**Nothing stops a backend from binding a provider's associated type to
something other than the matching `net-lattice-model` type** — that is an
unavoidable consequence of `net-lattice-platform` staying generic. A backend
crate is free to write `type Route = LinuxRoute;` for some
backend-specific type instead of `net_lattice_model::route::Route`. This is
not a gap to close by making `net-lattice-platform` depend on `net-lattice-model`
(see the previous section); it is closed one layer up, in `net-lattice`. See
below.

### `net-lattice`

The public-facing facade. Re-exports the types consumers need from
`net-lattice-model` and `net-lattice-ip`, selects a default backend based on
`cfg(target_os = "...")`, and exposes the top-level API (e.g.
`Lattice::connect()`). This is the only crate most consumers depend on
directly.

**This is where model convergence is enforced.** `net-lattice-platform`'s
generic contract means a backend's associated types could, in principle,
diverge from `net-lattice-model` (see the previous section). `net-lattice` closes
that gap not by adding a `net-lattice-platform → net-lattice-model` dependency,
but by constraining the associated types to equal the concrete
`net-lattice-model` types wherever it accepts a backend:

```rust
pub trait LatticeBackend:
    RouteProvider<Route = net_lattice_model::route::Route>
    + InterfaceProvider<Interface = net_lattice_model::interface::Interface>
{
}

pub struct Lattice<B: LatticeBackend> {
    backend: B,
}
```

A backend whose `Route` associated type is not literally
`net_lattice_model::route::Route` simply fails to satisfy `LatticeBackend` and
cannot be used with the public `Lattice` type — a compile error at the
point the backend is wired in, not a runtime surprise. Collecting the
per-provider bounds into one named `LatticeBackend` trait (rather than
repeating a growing `where` clause on `Lattice` itself) is purely
ergonomic — it does not change where the constraint lives. This gives the
same strength of guarantee as a direct dependency would, without requiring
`net-lattice-platform` itself to know `net-lattice-model` exists: the constraint
lives with the consumer of the contract (the facade that assembles a
concrete system), not with the contract's definition. This is the same
shape used by crates like `sqlx` and `diesel`, where a generic backend
trait is paired with a concrete type binding enforced at the point of use.

**This generic design trades away object safety, deliberately, for now.**
Associated types (`RouteProvider::Route`, ...) make these traits
unimplementable as `Box<dyn RouteProvider>` — Rust cannot build a vtable
for a trait whose method signatures depend on a type that varies per
implementor. Concretely, this means backends must be selected at compile
time (`Lattice<LinuxBackend>`) rather than chosen dynamically at runtime
from a list of loaded implementations. For Net Lattice's actual delivery plan
— a fixed, statically-linked backend per target OS, selected via
`cfg(target_os = "...")` — this costs nothing. It would matter if Lattice
later needed to pick between multiple competing backends for the same
platform at runtime (e.g. Netlink vs. a NetworkManager-based backend on
the same machine); if that need materializes, an object-safe erased layer
(non-generic `dyn`-compatible traits that internally forward to the
generic ones, conventionally called `DynRouteProvider` and friends) can be
added in `net-lattice-platform` without changing the generic traits consumers
already depend on. This is a reserved extension point, not a commitment —
it is not built until a concrete use case needs it.

## Error Model

Lattice must not leak `std::io::Error` or raw OS error codes
(`EPERM`/`ENODEV` on Linux, `ERROR_ACCESS_DENIED` on Windows) as its public
error type. Different backends fail for the same logical reason through
completely different codes, and a consumer writing cross-platform code
needs to match on *why* an operation failed, not on a platform-specific
integer.

`net-lattice-core::Error` is the single error type surfaced across the
workspace, expressed as platform-independent variants such as:

- `PermissionDenied`
- `NotFound`
- `AlreadyExists`
- `Unsupported` — the operation has no meaning on this backend at all (as
  opposed to a `Capability` being absent at runtime; see below).
- `InvalidState`
- `PlatformError` — an escape hatch that preserves the raw backend-specific
  error for diagnostics, without being the primary way consumers are
  expected to match on failures.

The exact variant list is an API design decision for the Stage 0.1 draft;
what this document fixes is that such a taxonomy exists and lives in
`net-lattice-core`, and that provider trait methods return `Result<T, Error>`
using it — never a raw OS error type.

**`PlatformError`'s code cannot be a single untyped integer.** Linux errno
is a signed `i32`, Windows error codes are an unsigned `DWORD` (`u32`), and
collapsing both into one bare `i32`/`u32` field either silently truncates
one of them or gives a false impression that codes are comparable across
platforms when they are not — a Linux `13` and a Windows `13` mean nothing
alike. The code must be tagged by platform, e.g. an enum
(`PlatformErrorCode::Linux(i32)` / `Windows(u32)` / `Darwin(i32)`) or a
boxed `dyn Error`. Either resolves the ambiguity; picking between them is a
Stage 0.1 decision, but leaving the code as one plain integer type is
ruled out here.

## Privilege Model

Networking configuration is privileged on every target platform, and the
privilege boundary does not line up the same way across them:

- **Linux** — reading routes/interfaces is generally unprivileged; adding
  or removing them requires `CAP_NET_ADMIN`.
- **Windows** — reading is available to normal users; modifying typically
  requires Administrator.
- **macOS** — similar read/write asymmetry via BSD route sockets.

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
Windows, BSD routing sockets on macOS). Its Stage 0.8 API is synchronous
and runtime-agnostic: `watch() -> Result<EventReceiver<Event>>`.
`EventReceiver` mirrors `std::sync::mpsc::Receiver`: it offers `recv`,
`try_recv`, and `recv_timeout`, and implements `Iterator`. This keeps the
core usable by synchronous programs and avoids imposing an async runtime.

The synchronous contract remains available without an async dependency. Stage
0.11 adds the optional `net-lattice` `async` feature, which re-exports the
single runtime-agnostic `net-lattice-async::EventStream` and adds
`Lattice::watch_async(filter)`. `EventStream` implements `futures::Stream`,
so applications remain free to choose an executor. This is not a zero-cost
wrapper around `EventReceiver`: `std::sync::mpsc::Receiver` cannot register a
waker. The separate async crate retains an explicit worker-thread bridge for
an arbitrary synchronous receiver, but the facade uses each backend's native
Tokio-aware path: Netlink is polled by Linux's existing Tokio runtime, Windows
IP Helper callbacks write to a bounded Tokio channel, and macOS's PF_ROUTE
reader thread writes directly to that channel. All native async transports use
the same bounded delivery and resynchronization semantics as `EventReceiver`.

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

- `SnapshotProvider` — a `net-lattice-platform` provider trait (generic over
  an associated `State` type, like the others) that assembles a
  `CurrentState` by reading the other providers a backend implements. This
  is the concrete mechanism behind `CurrentState` below, rather than each
  backend or the facade having to hand-assemble a snapshot ad hoc.
- `CurrentState` — the snapshot `SnapshotProvider` produces by reading
  providers (routes, interfaces, ...) for a given backend, built from
  `net-lattice-model`'s existing state types (`Route`, `Interface`, ...).
- `DesiredState` — **not the same type as `CurrentState`.** A desired route
  or interface is expressed as a distinct configuration type
  (`RouteConfig`, `InterfaceConfig`, ...) alongside the corresponding state
  type, not a reused `Route`/`Interface`. State objects carry fields that
  are read-only facts about the current system (an interface's live MTU,
  its operational state, traffic counters) which cannot be meaningfully
  "desired" — a consumer expressing intent should not be able to construct
  a `Route` with a nonsensical read-only field set, nor should the
  compiler let them try.
- `Diff` — the computed difference between a `CurrentState` and a
  `DesiredState`, comparing state and config types field-by-field where
  they overlap.
- `ApplyPlan` — an ordered sequence of provider calls (add/remove/modify)
  that would resolve a `Diff`, which can be inspected before being executed
  and rolled back if a step fails.

None of these types exist yet, and no crate is created for them now — they
belong to stages 0.18–0.20, after the transaction and remaining imperative
mutation contracts are stable enough to compute a meaningful diff against.
The state/config split is named here
— as a parallel `*Config` type per domain object living alongside its state
type in `net-lattice-model` — so that it is built in from the first `*Config`
type rather than retrofitted after `CurrentState`/`DesiredState` have
already been conflated into one type.

## Mutation and Event Contract Before Transactions

The current imperative API is useful, but it is not an atomic configuration
engine. Stage 0.14 turns the following observed behavior into explicit
operation metadata before a transaction API can make stronger promises.

| Domain | Current mutation contract | Required normalization before declarative apply |
|---|---|---|
| Routes | `RouteProvider` adds and removes through native acknowledgements. `Route` is currently both the observed record and mutation input; deletion matching is platform-specific. | Introduce a distinct route intent and an operation precondition/match rule. Define duplicate, absent, and ambiguous-match outcomes. |
| Interface addresses | `AddressMutator::add_address` returns a re-read `InterfaceAddress`; removal accepts that observed record. IDs are synthesized from interface and network rather than kernel-issued stable identities. | Record identity scope, collision assumptions, and removal preconditions in the operation model. |
| DNS | `DnsMutator` replaces the portable resolver view and re-reads `DnsConfig`. Unix rewrites the active resolver file and drops directives outside the portable model; Windows changes global search settings and each enumerated adapter through separate calls. | Surface scope, manager ownership, persistence, and partial-application results in the operation report. Do not promise atomic DNS replacement or automatic rollback. |
| Interface configuration | Read-only today. | Add a separate desired configuration for admin state and MTU; do not reuse observed `Interface`. |
| Neighbors | Read-only today. | Add distinct static-neighbor intent and lifecycle semantics before exposing mutation. |

Every future mutation operation must state: its target identity and matching
rule; preconditions; idempotent result; required privilege; whether the OS
acknowledges completion; whether Net Lattice re-reads observed state; whether
partial application is possible; and whether a compensating operation is safe.
`ApplyPlan` may use this metadata, but must never infer rollback safety from a
successful call alone.

Event delivery is deliberately a separate, eventually consistent signal path:

- A watcher does not provide an initial snapshot, a global order across
  domains, causal correlation with a caller's mutation, or a guarantee that a
  successful mutation produces an event. A snapshot comes from the read
  providers.
- Linux monitors routes, links, neighbors, and interface addresses through
  Netlink. Windows monitors routes, interfaces, and unicast addresses through
  IP Helper; it has no neighbor watcher. macOS monitors routes, interfaces,
  neighbors, and addresses through PF_ROUTE.
- No backend emits DNS events in Stage 0.13. DNS mutation must be followed by
  `dns_config()` when a caller needs the resulting view.
- A backend preserves its enqueue order, not a cross-domain total order. A
  full bounded queue coalesces loss into `Event::ResyncRequired`; consumers
  must re-read the indicated domain before interpreting subsequent ordinary
  events.
- Native sources frequently cannot distinguish create from modification.
  `ChangeKind::Changed` is therefore the conservative result where the OS
  does not provide an unambiguous lifecycle transition.

Stages 0.15–0.20 must build transactions and declarative apply on these
constraints rather than retroactively claiming atomicity or event guarantees
that the native sources do not provide.

## API Stability Rules

Once published, different crates in this workspace are expected to change
at different rates, and consumers need to know which promises hold at
which layer:

- **`net-lattice-core`** — the most stable crate in the workspace. `Error`,
  `Id<T>`, and shared traits are depended on by everything else; a breaking
  change here forces a breaking change everywhere. Changes require the
  strongest justification and the widest review.
- **`net-lattice-ip`** — stable once IPv4/IPv6 types are implemented; the
  domain (IP addressing) is well-understood and slow-moving.
- **`net-lattice-model`** — moderate stability. New modules (`dns`, `neighbor`,
  ...) are expected to be added over time per the delivery plan, but
  existing types should change conservatively once a domain has shipped,
  since both backends and consumers depend on their exact shape.
- **`net-lattice-platform`** — expected to evolve faster than `net-lattice-model`,
  since new provider traits are added as new domains gain backend support.
  Adding a trait is not breaking; changing an existing trait's signature
  is, and affects every backend that implements it.
- **`net-lattice-backend-*` crates** — the least stable. Internal
  implementation details may change freely; only the provider trait
  implementations they expose are a compatibility surface, and that
  surface is owned by `net-lattice-platform`, not the backend crate itself.

This ranking exists so that a change's blast radius can be reasoned about
before it's made, not so that any crate is exempt from normal semver
discipline once Lattice reaches 1.0.

## Explicit Non-Goals of This Architecture

- **No crate is Linux-, Windows-, or macOS-specific except the backend
  crates themselves.** `net-lattice-core`, `net-lattice-ip`, `net-lattice-model`, and
  `net-lattice-platform` must remain free of `cfg(target_os = "...")` and OS
  bindings.
- **`net-lattice-platform` never depends on `net-lattice-model`.** Its provider
  traits must stay generic over associated types rather than growing a
  direct dependency on concrete model types, even when it would be
  momentarily convenient (e.g. adding a new provider method whose most
  obvious signature names `net_lattice_model::route::Route` directly). If a
  provider trait cannot be expressed without naming a concrete model type,
  that is a signal to revisit the trait's shape, not to add the
  dependency.
- **No command-line interface.** Consistent with the project's non-goals in
  [README.md](README.md), no `net-lattice-cli` crate is planned.
- **No premature crate creation.** Crates for future domains (VLAN, VRF,
  firewall, tunnels, declarative configuration, transactional apply/rollback)
  are described in the roadmap below but are not created until there is
  actual code to put in them.

## Incremental Delivery Plan

The full model above is a target, not a starting point. Crates and modules
are introduced only when there is real implementation work for them:

| Stage | Scope |
|-------|-------|
| 0.1 ✅ | `net-lattice-core`, `net-lattice-ip`, `net-lattice-model` (`route` module only), `net-lattice-platform` (`RouteProvider`), `net-lattice-backend-linux` (routes via Netlink), `net-lattice` |
| 0.2 ✅ | `net-lattice-backend-windows` (`RouteProvider`) |
| 0.3 ✅ | `net-lattice-backend-darwin` (`RouteProvider`) |
| 0.4 ✅ | `interface` module + `InterfaceProvider` across all backends |
| 0.5 ✅ | `dns` module + `DnsProvider` across all backends |
| 0.6 ✅ | `neighbor` module + `NeighborProvider` (ARP/NDP) across all backends |
| 0.7 ✅ | `ifaddr` module + `AddressProvider` (IP addresses on interfaces) across all backends |
| 0.8 ✅ | `event` module + synchronous `EventProvider`/`EventReceiver`; monitoring via Netlink multicast (Linux), PF_ROUTE (macOS), and IP Helper notifications (Windows). |
| 0.9 ✅ | `NewInterfaceAddress` + `AddressMutator`; native IPv4/IPv6 address assignment/removal via Netlink (Linux), IP Helper (Windows), and address ioctls (macOS). |
| 0.10 ✅ | Event semantics: bounded delivery, overflow/resynchronization, filtering, cancellation, and background-error propagation. |
| 0.11 ✅ | Optional `net-lattice` `async` feature; `net-lattice-async` exposes one runtime-agnostic `EventStream`, while Linux (Tokio Netlink), Windows (IP Helper callbacks), and macOS (PF_ROUTE reader) deliver directly into bounded Tokio transports. |
| 0.12 ✅ | Watcher API stabilization: composable object/domain filters applied before enqueueing, monitoring-capability validation, and consistent synchronous/async filter semantics without changing the released 0.11 API. |
| 0.13 ✅ | DNS mutation with an intent/observed-state model: `NewDnsConfig` is applied through supported system mechanisms and the resulting `DnsConfig` is re-read on Linux, Windows, and macOS. |
| 0.14 ✅ | Mutation operation model: inspectable `Mutation` values and ordered `MutationPlan`s for existing route/address/DNS mutations; explicit preconditions, idempotency, privilege, confirmation, partial-application, and reversibility classifications. Adds typed `MutationOutcome`, `MutationPlanReport`, and `RollbackStatus` contracts for executor reporting, while plans themselves retain no execution or rollback side effects. |
| 0.15 | Transaction execution: ordered plans, per-operation outcomes, failure reporting, cancellation boundaries, and best-effort compensation only where an operation is documented as reversible. Native integration tests establish the same contract on Linux, Windows, and macOS. |
| 0.16 | Interface configuration: separate desired interface configuration from observed `Interface`; capability-gated admin-state and MTU mutation with platform parity and event semantics. |
| 0.17 | Neighbor mutation: intent/observed types and capability-gated static ARP/NDP entry management. This completes the mutation counterpart of the existing neighbor read model. |
| 0.18 | Snapshot foundation: `CurrentState` assembled consistently from the implemented providers, with snapshot scope, consistency, and partial-read semantics made explicit. |
| 0.19 | Declarative model and diff: `DesiredState` configuration types remain distinct from observed types; produce an inspectable `Diff` without applying it. |
| 0.20 | Declarative apply: compile a `Diff` into an `ApplyPlan`, execute it through the transaction engine, and report convergence, non-convergence, and compensation results. |
| 0.21 | Pre-1.0 hardening: freeze the core model, provider extension contracts, identity rules, capability meanings, event guarantees, and platform support matrix; complete cross-platform privileged regression coverage and migration guidance. |
| 0.22+ | Capability domains, each introduced only with its read model, intent model, mutation semantics, events where the OS supports them, capabilities, and all-platform tests: VLAN first, then VRF, namespaces, firewall, and tunnels as their platform contracts mature. These domains are not prerequisites for 1.0. |
| 1.0 | Stable cross-platform foundation for the implemented inspection, monitoring, imperative mutation, transactions, and declarative apply contracts. 1.0 is gated by the 0.21 compatibility audit, not by implementing every future capability domain. |

Each stage is expected to validate the architecture before the next is
started; earlier stages may inform adjustments to later ones.

### Route to 1.0

The stage numbers above are delivery boundaries, not a promise that every
heading ships in one release. A stage may be split when platform behavior or
the public contract needs independent validation. Conversely, a small
hardening release may be issued between stages without changing this plan.

The current facade exposes complete read APIs plus imperative route, address,
and DNS mutation. That is enough to begin transactions, but not enough to
declare a stable configuration platform: interface and static-neighbor
mutation still need their own intent models, and declarative apply must be
defined in terms of explicit operations rather than by reusing observed
objects as desired state.

The 1.0 boundary intentionally does not require VLAN, VRF, namespaces,
firewall, or tunnel support. It requires that every API already advertised as
stable has a documented cross-platform contract, truthful capability and
privilege behavior, bounded event semantics, deterministic transaction
reporting, and privileged regression coverage on each supported platform.
