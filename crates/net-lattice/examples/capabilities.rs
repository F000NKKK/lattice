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
        ("route monitoring", Capability::ROUTE_MONITORING),
        ("interface monitoring", Capability::INTERFACE_MONITORING),
        ("neighbor monitoring", Capability::NEIGHBOR_MONITORING),
        ("address monitoring", Capability::ADDRESS_MONITORING),
        ("all-domain monitoring", Capability::MONITORING),
        ("DNS mutation", Capability::DNS_MUTATION),
        ("interface administrative state", Capability::INTERFACE_ADMIN_STATE),
        ("interface MTU", Capability::INTERFACE_MTU),
        ("VRF", Capability::VRF),
        ("network namespaces", Capability::NAMESPACES),
    ] {
        println!("{name}: {}", lattice.supports(capability));
    }

    Ok(())
}
