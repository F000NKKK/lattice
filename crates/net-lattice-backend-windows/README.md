# net-lattice-backend-windows

Windows backend for Net Lattice using the IP Helper API. It implements the
generic provider contracts for routes, interfaces, addresses, DNS, neighbors,
and native notifications.

Applications normally use the `net-lattice` facade. Privileged mutation tests
run in the Windows CI job with the required administrator context.

## Example

```rust,no_run
let backend = net_lattice_backend_windows::WindowsBackend::new()?;
let interfaces = net_lattice_platform::InterfaceProvider::interfaces(&backend)?;
println!("{} interfaces", interfaces.len());
# Ok::<(), net_lattice_core::Error>(())
```
