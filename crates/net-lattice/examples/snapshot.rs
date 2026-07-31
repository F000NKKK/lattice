//! List a snapshot of the networking state without changing it.

use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    for interface in lattice.interfaces()? {
        println!("interface: {interface:?}");
    }
    for route in lattice.routes()? {
        println!("route: {route:?}");
    }
    for address in lattice.addresses()? {
        println!("address: {address:?}");
    }
    Ok(())
}
