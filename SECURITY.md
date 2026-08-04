# Security Policy

## Supported Versions

Net Lattice follows a rolling support policy. Security fixes are provided only
for the latest stable release series. The current supported line is 0.18.x;
support for the 0.1.x-0.17.x series has ended.

| Version | Supported |
| ------- | --------- |
| 0.18.x | ✅ |
| 0.1.x - 0.17.x | ❌ |

## Reporting a Vulnerability

If you discover a security vulnerability in Net Lattice, please **do not** open a
public GitHub issue.

Instead, report it privately using [GitHub's private vulnerability reporting](https://github.com/F000NKKK/net-lattice/security/advisories/new)
feature for this repository.

Please include as much of the following information as possible:

- A description of the vulnerability and its potential impact
- Steps to reproduce the issue
- Affected versions or commits, if known
- Any suggested mitigations

We will make a best effort to acknowledge reports promptly and to keep you
informed as the issue is investigated and resolved.

## Scope

The latest published release, Stage 0.18 of Net Lattice's
[architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan, provides route inspection and mutation, interface
inspection, DNS resolver inspection and mutation, neighbor inspection and
static ARP/NDP neighbor mutation, interface-address inspection and mutation,
capability-gated interface configuration, and whole-system `CurrentState`
snapshot assembly (`net-lattice-platform::SnapshotProvider`,
`Lattice::current_state()`, fail-fast on the first constituent read error, no
partial result) on Linux
(`net-lattice-backend-linux`, via Netlink and `/etc/resolv.conf`), Windows
(`net-lattice-backend-windows`, via the IP Helper API), and macOS
(`net-lattice-backend-darwin`, via BSD routing sockets, `getifaddrs`, address
ioctls, and `/etc/resolv.conf`). Monitoring is bounded with explicit overflow
resynchronization: Linux observes routes, links, neighbors, and addresses via
Netlink multicast; Windows observes routes, interfaces, and unicast addresses
via IP Helper; macOS observes routes, interfaces, neighbors, and addresses via
PF_ROUTE. DNS and static-neighbor changes do not currently produce watcher
events (static-neighbor mutation is a request/response native call, not an
event subscription). The model publishes inspectable, data-only mutation
plans for route, interface-address, DNS, and static-neighbor operations,
side-effect-free `MutationPreflight` analysis, and typed `MutationOutcome`,
`MutationPlanReport`, and `RollbackStatus` contracts. The executor adds
ordered plan submission, runtime preflight, operation-boundary cancellation,
typed prior-state snapshots, phase/timing reports, and explicit reverse-order
compensation. The executor never infers inverse operations or elevates
privileges on the caller's behalf.

Route, interface-address, DNS-mutation, and static-neighbor-mutation
operations are privileged (see [ARCHITECTURE.md](ARCHITECTURE.md)'s Privilege
Model), gated by `Capability::ROUTE_MUTATION`/`NEIGHBOR_MUTATION` respectively
(`net-lattice-platform`'s `RouteMutator`/`NeighborMutator`, split from the
combined `RouteProvider` in this release — see `ARCHITECTURE.md`'s Provider
trait section). Removing a static neighbor refuses to delete a present but
non-`Permanent` (dynamically learned) entry, returning `InvalidState`, so a
removal request cannot silently evict a dynamically learned ARP/NDP cache
entry. Reports involving unintended network mutation, partial DNS
application, privilege confusion, or memory-safety issues in route,
interface, DNS, neighbor, address, or monitoring message/data handling are
in scope. Firewall, VLAN, VRF, namespace, isolated destructive topology
orchestration, and tunnel domains do not exist yet.
