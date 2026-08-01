# net-lattice-backend-darwin

macOS backend for Net Lattice using BSD routing sockets, `getifaddrs`, and
native ioctls. It implements the generic `net-lattice-platform` contracts.

## What it provides

- interface, address, route, neighbor, and resolver inspection;
- route, address, and resolver mutation where macOS exposes portable
  semantics;
- routing-socket monitoring and optional async delivery;
- preservation of native error codes in the shared error model.

Applications should normally use the `net-lattice` facade, which selects this
backend automatically on macOS. Direct use is intended for backend integration
and diagnostics.

## Direct usage

```rust,no_run
use net_lattice_platform::InterfaceProvider;

fn main() -> net_lattice_core::Result<()> {
    let backend = net_lattice_backend_darwin::DarwinBackend::new()?;
    for interface in backend.interfaces()? {
        println!("{interface:?}");
    }
    Ok(())
}
```

## Privileges and safety

Inspection is normally unprivileged. Mutations can require root or specific
system entitlements and may interact with macOS network configuration
services. Privileged tests run separately and must restore changed state.
