# Security Policy

## Supported Versions

Net Lattice follows a rolling support policy. Security fixes are provided only
for the latest stable release series. With the release of Stage 0.9, support
for the 0.1.x-0.8.x series has ended — upgrade to 0.9.x to receive fixes.

| Version | Supported |
| ------- | --------- |
| 0.9.x | ✅ |
| 0.1.0 - 0.8.x | ❌ |

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

Net Lattice has landed Stage 0.9 of its [architecture](ARCHITECTURE.md)'s
Incremental Delivery Plan: route, interface, DNS-read, neighbor-read, and
address-read and address-mutation providers for Linux (`net-lattice-backend-linux`, via Netlink
and `/etc/resolv.conf`), Windows (`net-lattice-backend-windows`, via the IP
Helper API), and BSD/macOS (`net-lattice-backend-darwin`, via route sockets,
`getifaddrs`, address ioctls, and `/etc/resolv.conf`), plus monitoring via Netlink multicast
(Linux), PF_ROUTE (BSD/macOS), and IP Helper notifications (Windows). Route
Route, interface, and address-mutation operations are privileged (see ARCHITECTURE.md's Privilege
Model) — vulnerability reports involving unintended route manipulation,
privilege confusion, or memory-safety issues in route, interface, DNS,
neighbor, address, or monitoring message/data handling are in scope. No
other domain (firewall, ...) exists yet.
