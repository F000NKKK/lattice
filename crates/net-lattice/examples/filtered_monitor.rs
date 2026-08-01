//! Monitor selected event domains and, when available, individual objects.
//!
//! Run with `cargo run -p net-lattice --example filtered_monitor`.

use net_lattice::{Capability, Error, Event, EventFilter, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    // Domain selectors compose, but only request domains the backend can
    // actually deliver. Object selectors narrow only their own domain.
    let mut filter = EventFilter::none();
    if lattice.supports(Capability::ROUTE_MONITORING) {
        filter = filter.routes();
        if let Some(route) = lattice.routes()?.into_iter().next() {
            filter = filter.route(route.id);
        }
    }
    if lattice.supports(Capability::INTERFACE_MONITORING) {
        filter = filter.interfaces();
        if let Some(interface) = lattice.interfaces()?.into_iter().next() {
            filter = filter.interface(interface.id);
        }
    }
    if lattice.supports(Capability::NEIGHBOR_MONITORING) {
        filter = filter.neighbors();
        if let Some(neighbor) = lattice.neighbors()?.into_iter().next() {
            filter = filter.neighbor(neighbor.id);
        }
    }
    if lattice.supports(Capability::ADDRESS_MONITORING) {
        filter = filter.addresses();
        if let Some(address) = lattice.addresses()?.into_iter().next() {
            filter = filter.address(address.id);
        }
    }
    if filter.is_empty() {
        return Err(Error::Unsupported);
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
