# net-lattice-backend-linux

Linux backend for Net Lattice using Netlink and `/etc/resolv.conf` mechanisms.
It implements the generic provider contracts for routes, interfaces,
addresses, DNS, neighbors, monitoring, and future interface configuration.

Applications normally use the `net-lattice` facade rather than this crate
directly. Privileged mutation and watcher integration tests require Linux
networking capabilities in CI.

## Example

```rust,no_run
let backend = net_lattice_backend_linux::LinuxBackend::new()?;
let interfaces = net_lattice_platform::InterfaceProvider::interfaces(&backend)?;
println!("{} interfaces", interfaces.len());
# Ok::<(), net_lattice_core::Error>(())
```

Most applications should depend on the facade instead of constructing the
backend directly.
