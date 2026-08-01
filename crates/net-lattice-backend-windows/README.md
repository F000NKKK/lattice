# net-lattice-backend-windows

Windows backend for Net Lattice using the IP Helper API. It implements the
generic `net-lattice-platform` contracts through native Windows APIs.

## What it provides

- interface, address, route, neighbor, and resolver inspection;
- route, address, and resolver mutation where Windows exposes portable
  semantics;
- native change notifications and optional async delivery;
- preservation of Windows error codes in the shared error model.

Applications should normally use the `net-lattice` facade, which selects this
backend automatically on Windows. Direct use is intended for backend
integration and diagnostics.

## Direct usage

```rust,no_run
use net_lattice_platform::InterfaceProvider;

fn main() -> net_lattice_core::Result<()> {
    let backend = net_lattice_backend_windows::WindowsBackend::new()?;
    for interface in backend.interfaces()? {
        println!("{interface:?}");
    }
    Ok(())
}
```

## Privileges and safety

Inspection is normally unprivileged. Mutating host networking can require an
administrator context and remains subject to adapter and system policy.
Privileged tests run separately and must restore changed state.
