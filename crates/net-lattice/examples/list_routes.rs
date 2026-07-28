//! Lists the current system's routing table.
//!
//! Only reads the routing table (`RTM_GETROUTE`), which does not require
//! elevated privileges. Run with:
//!
//! ```sh
//! cargo run -p net-lattice --example list_routes
//! ```

use net_lattice::Lattice;

fn main() -> net_lattice::Result<()> {
    let lattice = Lattice::connect()?;

    for route in lattice.routes()? {
        println!("{route:?}");
    }

    Ok(())
}
