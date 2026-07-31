//! Inspect every currently readable networking domain without changing state.
//!
//! Run with `cargo run -p net-lattice --example snapshot`.

use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    println!("capabilities: {:?}", lattice.capabilities());
    for interface in lattice.interfaces()? {
        println!("interface: {interface:?}");
    }
    for route in lattice.routes()? {
        println!("route: {route:?}");
    }
    for address in lattice.addresses()? {
        println!("address: {address:?}");
    }
    println!("dns: {:?}", lattice.dns_config()?);
    for neighbor in lattice.neighbors()? {
        println!("neighbor: {neighbor:?}");
    }
    Ok(())
}
