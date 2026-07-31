//! Add and remove a test route when explicitly enabled by the caller.
//!
//! Set `NET_LATTICE_INTERFACE_INDEX` to a suitable egress interface and review
//! the hard-coded documentation prefix before running with elevated privilege.
//! The example removes only the route it successfully added.

use net_lattice::{
    Ipv4Address, Ipv4Network, Ipv4PrefixLength, Lattice, Network, Result, Route, RouteId,
};

fn main() -> Result<()> {
    let Some(interface_index) = std::env::var("NET_LATTICE_INTERFACE_INDEX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        eprintln!("set NET_LATTICE_INTERFACE_INDEX to run this mutation example");
        return Ok(());
    };

    let lattice = Lattice::connect()?;
    let route = Route::new(
        RouteId::new(0),
        Network::from(Ipv4Network::new(
            Ipv4Address::new(198, 18, 0, 0),
            Ipv4PrefixLength::new(15).expect("15 is a valid IPv4 prefix length"),
        )),
    )
    .with_interface_index(interface_index);

    lattice.add_route(route.clone())?;
    println!("added: {route:?}");
    lattice.remove_route(route)?;
    println!("removed the route created by this example");
    Ok(())
}
