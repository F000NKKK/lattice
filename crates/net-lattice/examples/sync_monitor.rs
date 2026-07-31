//! Receive synchronous network-change events and handle receiver outcomes.
//!
//! Run with `cargo run -p net-lattice --example sync_monitor` and change a
//! route or address from another terminal. `ResyncRequired` means the affected
//! provider state must be read again before later events are interpreted.

use std::time::Duration;

use net_lattice::{Event, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;
    let events = lattice.watch()?;

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
