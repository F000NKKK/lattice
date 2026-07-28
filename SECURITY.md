# Security Policy

## Supported Versions

Net Lattice has not yet published any releases. Once versioned releases begin, this
section will be updated to reflect which versions receive security fixes.

| Version | Supported |
| ------- | --------- |
| N/A (pre-release) | N/A |

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

Net Lattice has landed Stage 0.1 of its [architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan: a Linux route provider (`net-lattice-backend-linux`)
that reads and writes the kernel routing table via Netlink. This is a
privileged operation (see ARCHITECTURE.md's Privilege Model) — vulnerability
reports involving unintended route manipulation, privilege confusion, or
memory-safety issues in the Netlink message handling are in scope. No
other backend or domain (Windows, macOS, DNS, firewall, ...) exists yet.
