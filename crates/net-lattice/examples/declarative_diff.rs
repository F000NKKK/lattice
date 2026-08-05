//! Compute the declarative gap between observed and desired route state,
//! without applying anything.
//!
//! `DesiredState`/`Diff` are inspectable only as of this stage — see
//! `ARCHITECTURE.md`'s State Model section. `Diff::compute` never touches
//! the network; nothing here requires elevated privilege or changes host
//! state.
//!
//! Run with `cargo run -p net-lattice --example declarative_diff`.

use net_lattice::model::{CurrentState, Network};
use net_lattice::mutation::{DesiredState, Diff, RouteChange, RouteConfig};
use net_lattice::{Error, Ipv4Address, Ipv4Network, Ipv4PrefixLength, Lattice, Result};

fn main() -> Result<()> {
    let lattice = Lattice::connect()?;

    // Replace "eth0" with the actual name of the interface you want the
    // desired route to use — never assume the first/loopback interface.
    let target_name = "eth0";
    let interface = lattice
        .interfaces()?
        .into_iter()
        .find(|interface| interface.name == target_name)
        .ok_or(Error::NotFound)?;

    let current: CurrentState = lattice.current_state()?;

    // Manage only the `routes` domain: express exactly one desired route on
    // the selected interface, leave every other domain unmanaged (`None`)
    // so `Diff` proposes no changes for interfaces, neighbors, addresses,
    // or DNS regardless of what's currently observed there.
    let destination = Network::from(Ipv4Network::new(
        Ipv4Address::new(198, 51, 100, 0),
        Ipv4PrefixLength::new(24).expect("24 is a valid IPv4 prefix length"),
    ));
    let desired_route = RouteConfig::new(destination).with_interface_index(interface.index);
    let desired = DesiredState::empty().with_routes(vec![desired_route]);

    let diff = Diff::compute(&current, &desired);

    if diff.routes.is_empty() {
        println!("routes already match the desired state, nothing to do");
    }
    for change in &diff.routes {
        match change {
            RouteChange::Added(route) => println!("would add:    {route:?}"),
            RouteChange::Removed(route) => println!("would remove: {route:?}"),
            // `RouteChange` is `#[non_exhaustive]`: a future variant must
            // not silently fail to compile for existing callers.
            _ => println!("unrecognized route change: {change:?}"),
        }
    }
    // `diff.interfaces`/`.neighbors`/`.addresses`/`.dns` are guaranteed
    // empty/`None` here since `desired` left those domains unmanaged.
    Ok(())
}
