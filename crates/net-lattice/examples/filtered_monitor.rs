//! Monitor selected event domains and, when available, individual objects.
//!
//! Run with `cargo run -p net-lattice --example filtered_monitor`.

use net_lattice::{Event, EventFilter, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    // Domain selectors compose. Object selectors narrow only their own domain.
    let mut filter = EventFilter::none().interfaces().addresses().neighbors();
    if let Some(route) = lattice.routes()?.into_iter().next() {
        filter = filter.route(route.id);
    }
    if let Some(interface) = lattice.interfaces()?.into_iter().next() {
        filter = filter.interface(interface.id);
    }
    if let Some(neighbor) = lattice.neighbors()?.into_iter().next() {
        filter = filter.neighbor(neighbor.id);
    }
    if let Some(address) = lattice.addresses()?.into_iter().next() {
        filter = filter.address(address.id);
    }

    println!("filter: {filter:?}");
    for event in lattice.watch_filtered(filter)? {
        match event? {
            Event::ResyncRequired { domain } => {
                println!("re-read {domain:?} before using later events");
            }
            event => println!("event: {event:?}"),
        }
    }
    Ok(())
}
