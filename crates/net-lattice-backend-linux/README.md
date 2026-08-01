# net-lattice-backend-linux

Linux backend for Net Lattice using Netlink and `/etc/resolv.conf` mechanisms.
It implements native inspection, mutation, and monitoring behind the generic
`net-lattice-platform` contracts.

## What it provides

- interface, address, route, neighbor, and resolver inspection;
- route, address, resolver, and interface MTU/administrative-state mutation;
- Netlink change subscriptions and optional native async delivery;
- translation of native errors and state into portable Net Lattice types.

Applications should normally use the `net-lattice` facade, which selects this
backend automatically on Linux. Direct use is intended for backend integration
and diagnostics.

## Direct usage

```rust,no_run
use net_lattice_platform::InterfaceProvider;

fn main() -> net_lattice_core::Result<()> {
    let backend = net_lattice_backend_linux::LinuxBackend::new()?;
    for interface in backend.interfaces()? {
        println!("{interface:?}");
    }
    Ok(())
}
```

## Privileges and safety

Read-only operations generally require no elevated privilege. Interface,
address, and route changes require `CAP_NET_ADMIN`; resolver replacement also
depends on filesystem permissions and the host resolver manager. An interface
configuration request may carry MTU and administrative-state changes together,
which Linux can reject after applying one field, so callers must re-read state
after an error and use explicit transaction compensation if restoration is
required. Privileged tests are ignored in ordinary test runs and restore the
observed interface configuration on every exit path.
