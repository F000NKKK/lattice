//! Inspect runtime capabilities before choosing optional operations.
//!
//! Run with `cargo run -p net-lattice --example capabilities`.

use net_lattice::{Capability, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    let capabilities = lattice.capabilities();

    println!("all reported capabilities: {capabilities:?}");
    for (name, capability) in [
        ("IPv6", Capability::IPV6),
        ("monitoring", Capability::MONITORING),
        ("DNS mutation", Capability::DNS_MUTATION),
        ("VRF", Capability::VRF),
        ("network namespaces", Capability::NAMESPACES),
    ] {
        println!("{name}: {}", lattice.supports(capability));
    }

    Ok(())
}
