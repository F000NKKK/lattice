# net-lattice-backend-darwin

macOS backend for Net Lattice using BSD routing sockets, `getifaddrs`, and
native interface/address ioctls. It implements the generic provider contracts
for routes, interfaces, addresses, DNS, neighbors, and monitoring.

Applications normally use the `net-lattice` facade. Privileged mutation tests
run in the macOS CI job with the required permissions.

## Example

```rust,no_run
let backend = net_lattice_backend_darwin::DarwinBackend::new()?;
let interfaces = net_lattice_platform::InterfaceProvider::interfaces(&backend)?;
println!("{} interfaces", interfaces.len());
# Ok::<(), net_lattice_core::Error>(())
```
