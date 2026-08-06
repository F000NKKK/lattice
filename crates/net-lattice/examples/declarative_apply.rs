//! Declaratively converge one interface's routes to a desired state and
//! print the resulting [`net_lattice::mutation::ApplyPlanReport`].
//!
//! Unlike `declarative_diff` (which only computes and prints the gap),
//! [`net_lattice::Lattice::apply`] actually submits native mutations: it
//! chains [`net_lattice::Lattice::current_state`] → `Diff::compute` →
//! `ApplyPlan::compile` → [`net_lattice::Lattice::execute_apply_plan`] in
//! one call. Set `NET_LATTICE_INTERFACE_INDEX` to a suitable non-critical
//! interface and review the hard-coded destination below before running
//! with elevated privilege — this example only removes the route it added,
//! but a failed/partial apply can still leave state behind (see the report's
//! `rollback()` for whether compensation was attempted).
//!
//! Run with `cargo run -p net-lattice --example declarative_apply`.

use net_lattice::model::Network;
use net_lattice::mutation::{DesiredState, ExecutionOptions, RouteConfig};
use net_lattice::{Ipv4Address, Ipv4Network, Ipv4PrefixLength, Lattice, Result};

fn main() -> Result<()> {
    let Some(interface_index) = std::env::var("NET_LATTICE_INTERFACE_INDEX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        eprintln!("set NET_LATTICE_INTERFACE_INDEX to run this apply example");
        return Ok(());
    };

    let lattice = Lattice::connect()?;

    let destination = Network::from(Ipv4Network::new(
        Ipv4Address::new(198, 51, 100, 0),
        Ipv4PrefixLength::new(24).expect("24 is a valid IPv4 prefix length"),
    ));
    let desired_route = RouteConfig::new(destination).with_interface_index(interface_index);

    // Manage only the `routes` domain: every other domain stays unmanaged
    // (`None`), so `apply` proposes no changes for interfaces, neighbors,
    // addresses, or DNS regardless of what's currently observed there.
    let desired = DesiredState::empty().with_routes(vec![desired_route]);

    let mut options = ExecutionOptions::default();
    let report = lattice.apply(&desired, &mut options)?;

    println!(
        "applied {} of {} step(s); rollback: {:?}",
        report.applied_count(),
        report.len(),
        report.rollback()
    );
    for (index, outcome) in report.outcomes().iter().enumerate() {
        println!("  step {index}: {outcome:?}");
    }

    if report.is_success() {
        // Remove only the route this example added. Do NOT clean up via
        // `apply(&DesiredState::empty(), ..)`: an *unmanaged* (`None`)
        // routes domain proposes no removals at all (see
        // `DesiredState::empty`'s rustdoc), and `with_routes(vec![])` would
        // instead mark every domain-managed route for removal — including
        // routes this example never added. A direct, targeted
        // `remove_route` is the only safe cleanup here.
        lattice.remove_route(desired_route)?;
        println!("removed the route added by this example");
    }

    Ok(())
}
