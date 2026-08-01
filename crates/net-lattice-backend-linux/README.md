# net-lattice-backend-linux

Linux backend for Net Lattice using Netlink and `/etc/resolv.conf` mechanisms.
It implements the generic provider contracts for routes, interfaces,
addresses, DNS, neighbors, monitoring, and future interface configuration.

Applications normally use the `net-lattice` facade rather than this crate
directly. Privileged mutation and watcher integration tests require Linux
networking capabilities in CI.

