//! Replace and restore the portable DNS resolver view when explicitly enabled.
//!
//! Set `NET_LATTICE_APPLY_DNS_EXAMPLE=1` and run with elevated privilege. This
//! changes real system resolver configuration. The example attempts to restore
//! the initially observed portable view, but resolver managers or a failed
//! platform operation can still leave different state; inspect `dns_config()`
//! afterwards.

use net_lattice::model::IpAddress;
use net_lattice::mutation::NewDnsConfig;
use net_lattice::{Ipv4Address, Lattice, Result};

fn main() -> Result<()> {
    if std::env::var("NET_LATTICE_APPLY_DNS_EXAMPLE").as_deref() != Ok("1") {
        eprintln!("set NET_LATTICE_APPLY_DNS_EXAMPLE=1 to run this mutation example");
        return Ok(());
    }

    let lattice = Lattice::connect()?;
    let previous = lattice.dns_config()?;
    let requested = NewDnsConfig::with(
        vec![IpAddress::from(Ipv4Address::new(192, 0, 2, 53))],
        vec!["net-lattice.invalid".to_string()],
    );

    let replacement = lattice.set_dns_config(requested);
    let restore = lattice.set_dns_config(NewDnsConfig::with(
        previous.nameservers,
        previous.search_domains,
    ));

    println!("replacement result: {replacement:?}");
    println!("restore result: {restore:?}");
    println!("currently observed DNS: {:?}", lattice.dns_config()?);
    Ok(())
}
