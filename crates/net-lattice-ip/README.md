# net-lattice-ip

Strongly typed IPv4/IPv6 addresses, networks, and prefix lengths used by Net
Lattice domain models. The crate is platform-independent and provides display
and standard-library conversion helpers.

The published `net-lattice` facade provides the public API.

## Example

```rust
use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

let network = Ipv4Network::new(
    Ipv4Address::new(192, 0, 2, 0),
    Ipv4PrefixLength::new(24).expect("valid prefix"),
);
assert_eq!(network.to_string(), "192.0.2.0/24");
```
