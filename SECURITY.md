# Security Policy

## Supported Versions

Net Lattice follows a rolling support policy. Security fixes are provided only
for the latest stable release series.

| Version | Supported |
| ------- | --------- |
| 0.1.x | ✅ |
| 0.2.x | ✅ |

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

Net Lattice has landed Stage 0.1 and Stage 0.2 of its [architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan: route providers for Linux (`net-lattice-backend-linux`,
via Netlink) and Windows (`net-lattice-backend-windows`, via the IP Helper API).
These are privileged operations (see ARCHITECTURE.md's Privilege Model) — vulnerability
reports involving unintended route manipulation, privilege confusion, or
memory-safety issues in route message handling are in scope. No
other backend or domain (macOS, DNS, firewall, ...) exists yet.
