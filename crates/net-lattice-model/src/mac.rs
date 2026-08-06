use std::fmt;

/// A MAC (Ethernet hardware) address.
///
/// Stored as six octets in transmission order. No OS dependency — this is
/// pure data, used by `net-lattice-model`'s `interface` module and by
/// backends that read hardware addresses from the OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MacAddress([u8; 6]);

impl MacAddress {
    /// Creates a MAC address from six octets in transmission order.
    pub const fn new(octets: [u8; 6]) -> Self {
        Self(octets)
    }

    /// Returns the six octets in transmission order.
    pub const fn octets(&self) -> [u8; 6] {
        self.0
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a, b, c, d, e, g] = self.0;
        write!(f, "{a:02x}:{b:02x}:{c:02x}:{d:02x}:{e:02x}:{g:02x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_in_colon_hex() {
        let mac = MacAddress::new([0x02, 0x42, 0xab, 0xcd, 0xef, 0x01]);
        assert_eq!(mac.to_string(), "02:42:ab:cd:ef:01");
    }

    #[test]
    fn octets_round_trip() {
        let octets = [0xff; 6];
        let mac = MacAddress::new(octets);
        assert_eq!(mac.octets(), octets);
    }
}
