use std::fmt;
use std::net::Ipv6Addr;

use crate::Ipv6PrefixLength;

/// A typed IPv6 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv6Address(Ipv6Addr);

impl Ipv6Address {
    pub const fn new(segments: [u16; 8]) -> Self {
        Self(Ipv6Addr::new(
            segments[0],
            segments[1],
            segments[2],
            segments[3],
            segments[4],
            segments[5],
            segments[6],
            segments[7],
        ))
    }

    pub const fn segments(&self) -> [u16; 8] {
        self.0.segments()
    }
}

impl From<Ipv6Addr> for Ipv6Address {
    fn from(value: Ipv6Addr) -> Self {
        Self(value)
    }
}

impl From<Ipv6Address> for Ipv6Addr {
    fn from(value: Ipv6Address) -> Self {
        value.0
    }
}

impl fmt::Display for Ipv6Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An IPv6 network: an address paired with a prefix length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv6Network {
    address: Ipv6Address,
    prefix: Ipv6PrefixLength,
}

impl Ipv6Network {
    pub const fn new(address: Ipv6Address, prefix: Ipv6PrefixLength) -> Self {
        Self { address, prefix }
    }

    pub const fn address(&self) -> Ipv6Address {
        self.address
    }

    pub const fn prefix(&self) -> Ipv6PrefixLength {
        self.prefix
    }
}

impl fmt::Display for Ipv6Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix)
    }
}
