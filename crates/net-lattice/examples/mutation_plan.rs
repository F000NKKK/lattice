//! Build and inspect a Stage 0.14 mutation plan without changing the system.
//!
//! Run with `cargo run -p net-lattice --example mutation_plan`.
//!
//! This example never calls [`net_lattice::Lattice::connect`] and never
//! touches real host state; every value below (route, interface, address)
//! only ever exists as an in-memory [`net_lattice::mutation::MutationPlan`]
//! entry that is printed, not executed. The interface index `2` and
//! interface ID `2` below are therefore synthetic placeholders, unlike the
//! other examples in this directory (which read
//! `NET_LATTICE_INTERFACE_INDEX` at runtime specifically to avoid clobbering
//! an arbitrary real interface when actually applying a mutation to the
//! host).

use net_lattice::model::{InterfaceAddress, InterfaceAddressId, InterfaceId, IpAddress, Network};
use net_lattice::mutation::{
    Mutation, MutationPlan, NewDnsConfig, NewInterfaceAddress, RouteConfig,
};
use net_lattice::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

fn network(host: Ipv4Address) -> Network {
    Network::from(Ipv4Network::new(
        host,
        Ipv4PrefixLength::new(24).expect("24 is a valid IPv4 prefix length"),
    ))
}

fn main() {
    let route = RouteConfig::new(network(Ipv4Address::new(198, 18, 0, 0))).with_interface_index(2);
    let requested_address = NewInterfaceAddress::new(
        InterfaceId::new(2),
        network(Ipv4Address::new(192, 0, 2, 10)),
    );
    let observed_address = InterfaceAddress::new(
        InterfaceAddressId::new(1),
        2,
        network(Ipv4Address::new(192, 0, 2, 10)),
    );
    let dns = NewDnsConfig::with(
        vec![IpAddress::from(Ipv4Address::new(1, 1, 1, 1))],
        vec!["example.test".to_string()],
    );

    let plan = MutationPlan::from_operations([
        Mutation::AddRoute(route),
        Mutation::RemoveRoute(route),
        Mutation::AddAddress(requested_address),
        Mutation::RemoveAddress(observed_address),
        Mutation::SetDnsConfig(dns),
    ]);

    for operation in plan.operations() {
        println!("{operation:?}\n  semantics: {:?}", operation.semantics());
    }
    println!("{} operations; execution arrives in Stage 0.15", plan.len());
}
