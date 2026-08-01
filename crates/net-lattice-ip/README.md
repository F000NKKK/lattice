# net-lattice-ip

Strongly typed IPv4 and IPv6 primitives used by Net Lattice. This crate is
platform-independent and performs no network I/O.

## What it provides

- validated IPv4 and IPv6 prefix lengths;
- address and network types with canonical display formatting;
- conversions to and from the Rust standard library's IP types;
- family-safe APIs that keep IPv4 and IPv6 values distinct.

Applications using the full library normally import these types from
`net-lattice`; protocol tooling can depend on `net-lattice-ip` directly.

## Usage

```rust
use net_lattice_ip::{Ipv4Address, Ipv4Network, Ipv4PrefixLength};

let network = Ipv4Network::new(
    Ipv4Address::new(192, 0, 2, 0),
    Ipv4PrefixLength::new(24).expect("valid prefix"),
);
assert_eq!(network.to_string(), "192.0.2.0/24");
```

Invalid prefix lengths are rejected during construction rather than being
stored as malformed networks.
