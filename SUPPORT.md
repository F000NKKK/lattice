# Support

Thank you for your interest in Net Lattice.

## Getting Help

- **Questions and discussion:** Use [GitHub Discussions](https://github.com/F000NKKK/net-lattice/discussions) for general questions, ideas, and design discussion.
- **Bug reports and feature requests:** Use [GitHub Issues](https://github.com/F000NKKK/net-lattice/issues) with the appropriate issue template.
- **Security issues:** See [SECURITY.md](SECURITY.md) for the responsible disclosure process. Do not report security issues via public issues or discussions.

## Project Status

Net Lattice has completed Stage 0.15 of its [architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan: route and interface-address mutation, DNS resolver
inspection and mutation, interface and neighbor inspection, bounded
object/domain-filterable monitoring, optional native async event delivery, and
inspectable data-only mutation plans, side-effect-free `MutationPreflight`
analysis, and typed execution-report and compensation-boundary contracts for
the existing route, address, and DNS operations on Linux, Windows, and macOS.
Stage 0.16 interface configuration is verified by privileged Linux, Windows,
and macOS CI and is part of the published support surface. It remains a
partial imperative patch rather than a declarative desired-state model. Stage
0.17's IPv6 DNS parity, static ARP/NDP neighbor mutation (`NeighborMutator`,
`Capability::NEIGHBOR_MUTATION`), and isolated destructive topology testing
(route/address/neighbor CRUD, transaction compensation, filtered events, and
async delivery) are verified on privileged Linux, Windows, and macOS CI and
are part of the published support surface. `RouteProvider` was also split
into a read-only `RouteProvider` and a new `RouteMutator`
(`Capability::ROUTE_MUTATION`), matching the provider/mutator pattern used
by every other domain.
The Stage 0.15 executor provides ordered submission, runtime preflight,
operation-boundary cancellation, typed prior-state snapshot capture, phase and
timing reports, and an explicit compensator callback. Usage support is limited
to the published surface for now. See [README.md](README.md)'s Current Status;
questions about usage,
direction, and design are all welcome.
