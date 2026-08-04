//! Assign and then remove an IPv4 address when explicitly enabled by the caller.
//!
//! This example never changes network state unless `NET_LATTICE_INTERFACE_INDEX`
//! is set to the target interface's numeric index. It removes only the address
//! it successfully created. Choose an unused address appropriate for the host
//! before running it with sufficient privilege.

use net_lattice::model::Network;
use net_lattice::mutation::NewInterfaceAddress;
use net_lattice::{Error, Ipv4Address, Ipv4Network, Ipv4PrefixLength, Lattice, Result};

fn main() -> Result<()> {
    let Some(index) = std::env::var("NET_LATTICE_INTERFACE_INDEX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        eprintln!("set NET_LATTICE_INTERFACE_INDEX to assign the example address");
        return Ok(());
    };

    let lattice = Lattice::connect()?;
    let interface = lattice
        .interfaces()?
        .into_iter()
        .find(|interface| interface.index == index)
        .ok_or(Error::NotFound)?;
    let request = NewInterfaceAddress::new(
        interface.id,
        Network::from(Ipv4Network::new(
            Ipv4Address::new(192, 0, 2, 10),
            Ipv4PrefixLength::new(24).expect("24 is a valid IPv4 prefix length"),
        )),
    );
    let observed = lattice.add_address(request)?;
    println!("assigned: {observed:?}");
    lattice.remove_address(observed)?;
    println!("removed the address created by this example");
    Ok(())
}
