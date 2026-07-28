use std::fmt;
use std::net::Ipv4Addr;

use crate::Ipv4PrefixLength;

/// A typed IPv4 address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Address(Ipv4Addr);

impl Ipv4Address {
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self(Ipv4Addr::new(a, b, c, d))
    }

    pub const fn octets(&self) -> [u8; 4] {
        self.0.octets()
    }
}

impl From<Ipv4Addr> for Ipv4Address {
    fn from(value: Ipv4Addr) -> Self {
        Self(value)
    }
}

impl From<Ipv4Address> for Ipv4Addr {
    fn from(value: Ipv4Address) -> Self {
        value.0
    }
}

impl fmt::Display for Ipv4Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An IPv4 network: an address paired with a prefix length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4Network {
    address: Ipv4Address,
    prefix: Ipv4PrefixLength,
}

impl Ipv4Network {
    pub const fn new(address: Ipv4Address, prefix: Ipv4PrefixLength) -> Self {
        Self { address, prefix }
    }

    pub const fn address(&self) -> Ipv4Address {
        self.address
    }

    pub const fn prefix(&self) -> Ipv4PrefixLength {
        self.prefix
    }
}

impl fmt::Display for Ipv4Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_in_dotted_decimal() {
        assert_eq!(
            Ipv4Address::new(192, 168, 1, 10).to_string(),
            "192.168.1.10"
        );
    }

    #[test]
    fn network_displays_with_prefix() {
        let network = Ipv4Network::new(
            Ipv4Address::new(10, 0, 0, 0),
            Ipv4PrefixLength::new(24).unwrap(),
        );
        assert_eq!(network.to_string(), "10.0.0.0/24");
    }
}
