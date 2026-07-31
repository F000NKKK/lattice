//! Consume native change notifications through the optional async facade.

use futures::StreamExt;
use net_lattice::{EventFilter, Lattice, Result};

async fn monitor() -> Result<()> {
    let lattice = Lattice::connect()?;
    let mut events = lattice.watch_async(EventFilter::ALL)?;
    while let Some(event) = events.next().await {
        println!("event: {:?}", event?);
    }
    Ok(())
}

fn main() -> Result<()> {
    futures::executor::block_on(monitor())
}
