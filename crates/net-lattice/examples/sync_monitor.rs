//! Receive synchronous network-change events and handle receiver outcomes.
//!
//! Run with `cargo run -p net-lattice --example sync_monitor` and change a
//! route or address from another terminal. `ResyncRequired` means the affected
//! provider state must be read again before later events are interpreted.

use std::time::Duration;

use net_lattice::monitoring::{Event, EventFilter};
use net_lattice::{Capability, Error, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    // An all-domain `watch()` requires aggregate `MONITORING`. Select one
    // domain the connected backend actually advertises so this example also
    // works on Windows, whose IP Helper watcher has no neighbor callback.
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
    let events = lattice.watch_filtered(filter)?;

    loop {
        match events.recv_timeout(Duration::from_secs(30))? {
            Some(Event::ResyncRequired { domain }) => {
                println!("events were lost for {domain:?}; re-read that domain");
            }
            Some(event) => println!("event: {event:?}"),
            None => println!("no event in the last 30 seconds"),
        }
    }
}
