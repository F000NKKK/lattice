# Contributing to Lattice

Thank you for your interest in contributing to Lattice. This document describes how to get involved.

## Project Status

Lattice is currently in the **bootstrap stage**. The repository contains no crates or source code yet, and no API design has been finalized. Substantial code contributions are not expected at this stage — the most valuable contributions right now are:

- Feedback on the project's vision, scope, and roadmap (see [README.md](README.md))
- Discussion of API design and architecture once design work begins
- Documentation and tooling improvements

Please check open issues and discussions before starting significant work, to avoid duplicated effort.

## Getting Started

1. Fork the repository and clone your fork.
2. Create a topic branch for your change.
3. Make your changes, following the conventions described below.
4. Open a pull request against `main` using the provided pull request template.

## Development Conventions

Once implementation begins, Lattice will follow standard Rust ecosystem conventions:

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

By contributing to Lattice, you agree that your contributions will be licensed under the [Mozilla Public License 2.0](LICENSE).
