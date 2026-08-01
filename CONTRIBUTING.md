# Contributing to Net Lattice

Thank you for your interest in contributing to Net Lattice. This document describes how to get involved.

## Project Status

Net Lattice has landed Stage 0.14 of its [architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan: route and interface-address mutation, DNS resolver
inspection and mutation, interface and neighbor inspection, bounded
object/domain-filterable monitoring, optional native async event delivery, and
inspectable data-only mutation plans, side-effect-free `MutationPreflight`
analysis, and typed execution-report and compensation-boundary contracts for
the existing route, address, and DNS operations on Linux, Windows, and macOS.
Large capability domains remain ahead, but the published read, mutation,
monitoring, planning, and backend-extension APIs are real public surface. The
most valuable
contributions right now are:

- Feedback on the project's vision, scope, and roadmap (see [README.md](README.md))
- Discussion of API design and architecture for upcoming stages
- Implementation work on the next stages in [ARCHITECTURE.md](ARCHITECTURE.md)'s delivery plan (transaction execution, interface and neighbor mutation, declarative configuration, ...)
- Documentation and tooling improvements

Please read [ARCHITECTURE.md](ARCHITECTURE.md) before proposing a new crate, module, or provider trait — it documents the dependency rules (e.g. `net-lattice-platform` never depends on `net-lattice-model`) and the staged delivery order this project follows.

Please check open issues and discussions before starting significant work, to avoid duplicated effort.

## Getting Started

1. Fork the repository and clone your fork.
2. Create a topic branch for your change.
3. Make your changes, following the conventions described below.
4. Open a pull request against `main` using the provided pull request template.

## Development Conventions

Net Lattice follows standard Rust ecosystem conventions:

- Code must be formatted with `rustfmt`.
- Code must be free of `clippy` warnings.
- Public APIs must be documented.
- Changes must include appropriate tests.
- Commit messages should be clear and descriptive.

## Reporting Issues

Please use the issue templates under `.github/ISSUE_TEMPLATE/` when filing bug reports or feature requests. Include as much context as possible.

## Security Issues

Do not report security vulnerabilities through public GitHub issues. See [SECURITY.md](SECURITY.md) for the responsible disclosure process.

## Code of Conduct

By participating in this project, you agree to abide by the [Code of Conduct](CODE_OF_CONDUCT.md).

## License

By contributing to Net Lattice, you agree that your contributions will be licensed under the [Mozilla Public License 2.0](LICENSE).
