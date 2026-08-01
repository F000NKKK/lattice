# Support

Thank you for your interest in Net Lattice.

## Getting Help

- **Questions and discussion:** Use [GitHub Discussions](https://github.com/F000NKKK/net-lattice/discussions) for general questions, ideas, and design discussion.
- **Bug reports and feature requests:** Use [GitHub Issues](https://github.com/F000NKKK/net-lattice/issues) with the appropriate issue template.
- **Security issues:** See [SECURITY.md](SECURITY.md) for the responsible disclosure process. Do not report security issues via public issues or discussions.

## Project Status

Net Lattice has landed Stage 0.14 of its [architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan: route and interface-address mutation, DNS resolver
inspection and mutation, interface and neighbor inspection, bounded
object/domain-filterable monitoring, optional native async event delivery, and
inspectable data-only mutation plans, side-effect-free `MutationPreflight`
analysis, and typed execution-report and compensation-boundary contracts for
the existing route, address, and DNS operations on Linux, Windows, and macOS.
The Stage 0.15 executor now provides ordered submission, operation-boundary
cancellation, and an explicit compensator callback; automatic prior-state
snapshot capture remains future-stage work. Usage support is limited to the
published surface for now —
see [README.md](README.md)'s Current Status — but questions about usage,
direction, and design are all welcome.
