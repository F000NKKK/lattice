//! Consume native change notifications through the optional async facade.

use futures::StreamExt;
use net_lattice::monitoring::EventFilter;
use net_lattice::{Capability, Error, Lattice, Result};

async fn monitor() -> Result<()> {
    let lattice = Lattice::connect()?;
    // Use one advertised domain rather than assuming all four are delivered.
    let filter = if lattice.supports(Capability::ROUTE_MONITORING) {
        EventFilter::none().routes()
    } else if lattice.supports(Capability::INTERFACE_MONITORING) {
        EventFilter::none().interfaces()
    } else if lattice.supports(Capability::ADDRESS_MONITORING) {
        EventFilter::none().addresses()
    } else if lattice.supports(Capability::NEIGHBOR_MONITORING) {
        EventFilter::none().neighbors()
    } else {
        return Err(Error::Unsupported);
    };
    let mut events = lattice.watch_async(filter)?;
    while let Some(event) = events.next().await {
        println!("event: {:?}", event?);
    }
    Ok(())
}

fn main() -> Result<()> {
    futures::executor::block_on(monitor())
}
