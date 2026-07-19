use std::fmt;

use thiserror::Error;

use crate::PROTOCOL_VERSION;

pub const SERVICE_UUID: &str = "A1270550-41B2-4055-9E10-A1270550C0DE";
pub const ADVERTISEMENT_SIZE: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Platform {
    Ios = 0,
    Android = 1,
    Windows = 2,
    MacOs = 3,
    Linux = 4,
}

impl TryFrom<u8> for Platform {
    type Error = AdvertisementError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Ios),
            1 => Ok(Self::Android),
            2 => Ok(Self::Windows),
            3 => Ok(Self::MacOs),
            4 => Ok(Self::Linux),
            value => Err(AdvertisementError::UnknownPlatform(value)),
        }
    }
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct Capabilities(u16);

impl Capabilities {
    const WIFI_AWARE: u16 = 1 << 0;
    const WIFI_DIRECT_CONCURRENT: u16 = 1 << 1;
    const AP_HOST: u16 = 1 << 2;
    const LAN_CONNECTED: u16 = 1 << 3;

    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn wifi_aware(self) -> bool {
        self.contains(Self::WIFI_AWARE)
    }

    #[must_use]
    pub const fn wifi_direct_concurrent(self) -> bool {
        self.contains(Self::WIFI_DIRECT_CONCURRENT)
    }

    #[must_use]
    pub const fn ap_host(self) -> bool {
        self.contains(Self::AP_HOST)
    }

    #[must_use]
    pub const fn lan_connected(self) -> bool {
        self.contains(Self::LAN_CONNECTED)
    }

    #[must_use]
    pub const fn with_wifi_aware(mut self) -> Self {
        self.0 |= Self::WIFI_AWARE;
        self
    }

    #[must_use]
    pub const fn with_wifi_direct_concurrent(mut self) -> Self {
        self.0 |= Self::WIFI_DIRECT_CONCURRENT;
        self
    }

    #[must_use]
    pub const fn with_ap_host(mut self) -> Self {
        self.0 |= Self::AP_HOST;
        self
    }

    #[must_use]
    pub const fn with_lan_connected(mut self) -> Self {
        self.0 |= Self::LAN_CONNECTED;
        self
    }

    const fn contains(self, flag: u16) -> bool {
        self.0 & flag != 0
    }
}

impl fmt::Debug for Capabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Capabilities")
            .field("wifi_aware", &self.wifi_aware())
            .field("wifi_direct_concurrent", &self.wifi_direct_concurrent())
            .field("ap_host", &self.ap_host())
            .field("lan_connected", &self.lan_connected())
            .field("raw", &self.0)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Advertisement {
    pub protocol_version: u8,
    pub session_device_id: [u8; 8],
    pub capabilities: Capabilities,
    pub platform: Platform,
}

impl Advertisement {
    /// Creates an advertisement with a random session-scoped device ID.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot provide secure random
    /// bytes.
    pub fn new(platform: Platform, capabilities: Capabilities) -> Result<Self, getrandom::Error> {
        let mut session_device_id = [0_u8; 8];
        getrandom::fill(&mut session_device_id)?;

        Ok(Self {
            protocol_version: PROTOCOL_VERSION,
            session_device_id,
            capabilities,
            platform,
        })
    }

    #[must_use]
    pub fn encode(self) -> [u8; ADVERTISEMENT_SIZE] {
        let mut bytes = [0_u8; ADVERTISEMENT_SIZE];
        bytes[0] = self.protocol_version;
        bytes[1..9].copy_from_slice(&self.session_device_id);
        bytes[9..11].copy_from_slice(&self.capabilities.bits().to_be_bytes());
        bytes[11] = self.platform as u8;
        bytes
    }

    /// Decodes and validates a complete Service Data payload.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid length, unsupported protocol version, or
    /// unknown platform value.
    pub fn decode(bytes: &[u8]) -> Result<Self, AdvertisementError> {
        if bytes.len() != ADVERTISEMENT_SIZE {
            return Err(AdvertisementError::InvalidLength(bytes.len()));
        }

        let protocol_version = bytes[0];
        if protocol_version != PROTOCOL_VERSION {
            return Err(AdvertisementError::UnsupportedVersion(protocol_version));
        }

        let mut session_device_id = [0_u8; 8];
        session_device_id.copy_from_slice(&bytes[1..9]);

        Ok(Self {
            protocol_version,
            session_device_id,
            capabilities: Capabilities::from_bits(u16::from_be_bytes([bytes[9], bytes[10]])),
            platform: Platform::try_from(bytes[11])?,
        })
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum AdvertisementError {
    #[error("advertisement is {0} bytes; expected {ADVERTISEMENT_SIZE}")]
    InvalidLength(usize),
    #[error("protocol version {0} is not supported")]
    UnsupportedVersion(u8),
    #[error("platform value {0} is not recognized")]
    UnknownPlatform(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advertisement_round_trips() {
        let advertisement = Advertisement {
            protocol_version: PROTOCOL_VERSION,
            session_device_id: [1, 2, 3, 4, 5, 6, 7, 8],
            capabilities: Capabilities::default()
                .with_wifi_aware()
                .with_ap_host()
                .with_lan_connected(),
            platform: Platform::Android,
        };

        let encoded = advertisement.encode();

        assert_eq!(encoded.len(), ADVERTISEMENT_SIZE);
        assert_eq!(Advertisement::decode(&encoded), Ok(advertisement));
    }

    #[test]
    fn unknown_capability_bits_are_preserved() {
        let capabilities = Capabilities::from_bits(0x8011);
        let advertisement = Advertisement {
            protocol_version: PROTOCOL_VERSION,
            session_device_id: [9; 8],
            capabilities,
            platform: Platform::Linux,
        };

        let decoded = Advertisement::decode(&advertisement.encode()).unwrap();

        assert_eq!(decoded.capabilities.bits(), 0x8011);
        assert!(decoded.capabilities.wifi_aware());
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let mut bytes = [0_u8; ADVERTISEMENT_SIZE];
        bytes[0] = PROTOCOL_VERSION + 1;
        bytes[11] = Platform::Linux as u8;

        assert_eq!(
            Advertisement::decode(&bytes),
            Err(AdvertisementError::UnsupportedVersion(PROTOCOL_VERSION + 1))
        );
    }
}
