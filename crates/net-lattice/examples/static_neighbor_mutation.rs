//! Add and remove a static ARP/NDP neighbor entry when explicitly enabled by
//! the caller.
//!
//! Set `NET_LATTICE_INTERFACE_INDEX` to a suitable interface and review the
//! hard-coded documentation-range address and locally-administered MAC
//! before running with elevated privilege. The example removes only the
//! entry it successfully added.

use net_lattice::{
    Capability, Error, IpAddress, Ipv4Address, Lattice, MacAddress, Result, StaticNeighbor,
};

fn main() -> Result<()> {
    let Some(index) = std::env::var("NET_LATTICE_INTERFACE_INDEX")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
    else {
        eprintln!("set NET_LATTICE_INTERFACE_INDEX to run this mutation example");
        return Ok(());
    };

    let lattice = Lattice::connect()?;
    if !lattice.supports(Capability::NEIGHBOR_MUTATION) {
        eprintln!("connected backend does not advertise Capability::NEIGHBOR_MUTATION");
        return Ok(());
    }
    let interface = lattice
        .interfaces()?
        .into_iter()
        .find(|interface| interface.index == index)
        .ok_or(Error::NotFound)?;
    let neighbor = StaticNeighbor::new(
        interface.id,
        IpAddress::from(Ipv4Address::new(192, 0, 2, 250)),
        MacAddress::new([0x02, 0x00, 0x00, 0x00, 0x00, 0xfa]),
    );

    let observed = lattice.add_static_neighbor(neighbor)?;
    println!("added: {observed:?}");
    lattice.remove_static_neighbor(neighbor)?;
    println!("removed the static neighbor created by this example");
    Ok(())
}
