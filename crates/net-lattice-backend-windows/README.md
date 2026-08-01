# net-lattice-backend-windows

Windows backend for Net Lattice using the IP Helper API. It implements the
generic `net-lattice-platform` contracts through native Windows APIs.

## What it provides

- interface, address, route, neighbor, and resolver inspection;
- route, address, and resolver mutation where Windows exposes portable
  semantics;
- interface MTU and administrative-state patches, with a fresh observed
  readback after every successful native submission;
- native route, interface, and unicast-address notifications with optional
  async delivery;
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

Inspection is normally unprivileged. Mutating host networking, including an
interface MTU or administrative state, can require an administrator context
and remains subject to adapter and system policy. Windows applies MTU to its
applicable IPv4/IPv6 interface rows and administrative state through a
separate native operation, so a combined patch can be partially applied after
an error; callers must re-read state and use explicit compensation where
needed. IP Helper has no native neighbor-table change callback, so this backend
advertises route/interface/address monitoring capabilities only and rejects
neighbor or all-domain watcher requests before registration. The facade does
not synthesize events. Privileged tests run separately and must restore changed
state.
