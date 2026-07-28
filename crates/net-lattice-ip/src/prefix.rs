use std::fmt;

/// A validated IPv4 prefix length (0..=32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv4PrefixLength(u8);

impl Ipv4PrefixLength {
    pub const MAX: u8 = 32;

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(&self) -> u8 {
        self.0
    }
}

impl fmt::Display for Ipv4PrefixLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A validated IPv6 prefix length (0..=128).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Ipv6PrefixLength(u8);

impl Ipv6PrefixLength {
    pub const MAX: u8 = 128;

    pub const fn new(value: u8) -> Option<Self> {
        if value <= Self::MAX {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn value(&self) -> u8 {
        self.0
    }
}

impl fmt::Display for Ipv6PrefixLength {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipv4_prefix_accepts_0_to_32() {
        assert!(Ipv4PrefixLength::new(0).is_some());
        assert!(Ipv4PrefixLength::new(32).is_some());
        assert!(Ipv4PrefixLength::new(33).is_none());
    }

    #[test]
    fn ipv6_prefix_accepts_0_to_128() {
        assert!(Ipv6PrefixLength::new(0).is_some());
        assert!(Ipv6PrefixLength::new(128).is_some());
        assert!(Ipv6PrefixLength::new(129).is_none());
    }
}
