# net-lattice-backend-windows

Windows backend for Net Lattice using the IP Helper API. It implements the
generic provider contracts for routes, interfaces, addresses, DNS, neighbors,
and native notifications.

Applications normally use the `net-lattice` facade. Privileged mutation tests
run in the Windows CI job with the required administrator context.

