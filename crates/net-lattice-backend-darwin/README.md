# net-lattice-backend-darwin

macOS backend for Net Lattice using BSD routing sockets, `getifaddrs`, and
native interface/address ioctls. It implements the generic provider contracts
for routes, interfaces, addresses, DNS, neighbors, and monitoring.

Applications normally use the `net-lattice` facade. Privileged mutation tests
run in the macOS CI job with the required permissions.

