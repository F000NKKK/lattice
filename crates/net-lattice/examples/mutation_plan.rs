//! Build and inspect a Stage 0.14 mutation plan without changing the system.
//!
//! Run with `cargo run -p net-lattice --example mutation_plan`.

use net_lattice::{
    InterfaceAddress, InterfaceAddressId, InterfaceId, IpAddress, Ipv4Address, Ipv4Network,
    Ipv4PrefixLength, Mutation, MutationPlan, Network, NewDnsConfig, NewInterfaceAddress, Route,
    RouteId,
};

fn network(host: Ipv4Address) -> Network {
    Network::from(Ipv4Network::new(
        host,
        Ipv4PrefixLength::new(24).expect("24 is a valid IPv4 prefix length"),
    ))
}

fn main() {
    let route = Route::new(RouteId::new(0), network(Ipv4Address::new(198, 18, 0, 0)))
        .with_interface_index(2);
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
        Mutation::AddRoute(route.clone()),
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
