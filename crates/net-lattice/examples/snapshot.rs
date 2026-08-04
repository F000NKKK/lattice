//! Inspect every currently readable networking domain without changing state.
//!
//! Run with `cargo run -p net-lattice --example snapshot`.

use net_lattice::{Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    println!("capabilities: {:?}", lattice.capabilities());

    // `current_state()` assembles all five domains below in one fail-fast
    // call; the constituent reads are still sequential and independent (no
    // cross-domain atomicity), so this is equivalent to calling each of
    // `interfaces()`, `routes()`, `addresses()`, `dns_config()`, and
    // `neighbors()` individually.
    let state = lattice.current_state()?;
    for interface in &state.interfaces {
        println!("interface: {interface:?}");
    }
    for route in &state.routes {
        println!("route: {route:?}");
    }
    for address in &state.addresses {
        println!("address: {address:?}");
    }
    println!("dns: {:?}", state.dns);
    for neighbor in &state.neighbors {
        println!("neighbor: {neighbor:?}");
    }
    Ok(())
}
