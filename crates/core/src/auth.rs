use std::{fmt, io::Cursor};

use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::PROTOCOL_VERSION;

const KEY_SIZE: usize = 32;
const DERIVED_KEY_SIZE: usize = KEY_SIZE * 2;
const HKDF_INFO: &[u8] = b"airtoss v1";

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    ClientA,
    ServerB,
}

impl Side {
    const fn confirmation_label(self) -> &'static [u8] {
        match self {
            Self::ClientA => b"confirm-a",
            Self::ServerB => b"confirm-b",
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Hello {
    pub version: u8,
    pub session_device_id: [u8; 8],
    #[serde(rename = "eph_pub")]
    #[serde(with = "serde_bytes")]
    pub ephemeral_public_key: [u8; KEY_SIZE],
}

impl Hello {
    /// Decodes one complete CBOR Hello message.
    ///
    /// # Errors
    ///
    /// Returns an error when the message is invalid CBOR, has the wrong shape,
    /// or contains bytes after the first message.
    pub fn decode(raw: &[u8]) -> Result<Self, AuthError> {
        let mut reader = Cursor::new(raw);
        let hello = ciborium::from_reader(&mut reader)
            .map_err(|error| AuthError::InvalidHello(error.to_string()))?;

        if usize::try_from(reader.position()).ok() != Some(raw.len()) {
            return Err(AuthError::TrailingHelloData);
        }

        Ok(hello)
    }

    fn encode(&self) -> Result<Vec<u8>, AuthError> {
        let mut bytes = Vec::new();
        ciborium::into_writer(self, &mut bytes)
            .map_err(|error| AuthError::HelloEncoding(error.to_string()))?;
        Ok(bytes)
    }
}

pub struct PendingHandshake {
    local_hello_raw: Vec<u8>,
    local_session_device_id: [u8; 8],
    secret: StaticSecret,
}

impl PendingHandshake {
    /// Creates a fresh ephemeral key and its serialized Hello message.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system cannot provide secure random
    /// bytes or the Hello message cannot be encoded.
    pub fn start(session_device_id: [u8; 8]) -> Result<Self, AuthError> {
        let mut secret_bytes = [0_u8; KEY_SIZE];
        getrandom::fill(&mut secret_bytes).map_err(|error| AuthError::Random(error.to_string()))?;
        let secret = StaticSecret::from(secret_bytes);
        secret_bytes.zeroize();
        let public_key = PublicKey::from(&secret);
        let hello = Hello {
            version: PROTOCOL_VERSION,
            session_device_id,
            ephemeral_public_key: public_key.to_bytes(),
        };

        Ok(Self {
            local_hello_raw: hello.encode()?,
            local_session_device_id: session_device_id,
            secret,
        })
    }

    #[must_use]
    pub fn hello_raw(&self) -> &[u8] {
        &self.local_hello_raw
    }

    /// Derives the session authentication key from the peer's raw Hello.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed or incompatible Hello messages, duplicate
    /// session IDs, invalid peer keys, or a key derivation failure.
    pub fn finish(self, peer_hello_raw: &[u8], local_side: Side) -> Result<SessionKeys, AuthError> {
        let peer_hello = Hello::decode(peer_hello_raw)?;
        if peer_hello.version != PROTOCOL_VERSION {
            return Err(AuthError::UnsupportedVersion(peer_hello.version));
        }
        if peer_hello.session_device_id == self.local_session_device_id {
            return Err(AuthError::DuplicateSessionDeviceId);
        }

        let peer_public_key = PublicKey::from(peer_hello.ephemeral_public_key);
        let shared_secret = self.secret.diffie_hellman(&peer_public_key);
        if shared_secret.as_bytes().iter().all(|byte| *byte == 0) {
            return Err(AuthError::NonContributoryPublicKey);
        }

        let transcript = transcript_hash(local_side, &self.local_hello_raw, peer_hello_raw);
        let hkdf = Hkdf::<Sha256>::new(Some(&transcript), shared_secret.as_bytes());
        let mut derived = [0_u8; DERIVED_KEY_SIZE];
        hkdf.expand(HKDF_INFO, &mut derived)
            .map_err(|_| AuthError::KeyDerivation)?;

        let mut auth_key = [0_u8; KEY_SIZE];
        auth_key.copy_from_slice(&derived[..KEY_SIZE]);
        let verification_code = VerificationCode::from_key(&derived[KEY_SIZE..]);
        derived.zeroize();

        Ok(SessionKeys {
            auth_key,
            transcript,
            verification_code,
        })
    }
}

fn transcript_hash(local_side: Side, local_raw: &[u8], peer_raw: &[u8]) -> [u8; KEY_SIZE] {
    let mut digest = Sha256::new();
    match local_side {
        Side::ClientA => {
            digest.update(local_raw);
            digest.update(peer_raw);
        }
        Side::ServerB => {
            digest.update(peer_raw);
            digest.update(local_raw);
        }
    }
    digest.finalize().into()
}

#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SessionKeys {
    auth_key: [u8; KEY_SIZE],
    transcript: [u8; KEY_SIZE],
    #[zeroize(skip)]
    verification_code: VerificationCode,
}

impl SessionKeys {
    #[must_use]
    pub const fn verification_code(&self) -> VerificationCode {
        self.verification_code
    }

    /// Encodes the confirmation sent after the local user accepts the code.
    ///
    /// # Errors
    ///
    /// Returns an error when CBOR serialization fails.
    pub fn confirmation(&self, side: Side) -> Result<Vec<u8>, AuthError> {
        let confirmation = Confirm {
            mac: self.confirmation_mac(side),
        };
        let mut raw = Vec::new();
        ciborium::into_writer(&confirmation, &mut raw)
            .map_err(|error| AuthError::ConfirmEncoding(error.to_string()))?;
        Ok(raw)
    }

    /// Verifies the peer confirmation for the expected protocol side.
    ///
    /// # Errors
    ///
    /// Returns an error when the message is invalid CBOR, has the wrong shape,
    /// or contains trailing bytes.
    pub fn verify_confirmation(&self, side: Side, raw: &[u8]) -> Result<bool, AuthError> {
        let confirmation = Confirm::decode(raw)?;
        let mut mac = self.hmac();
        mac.update(side.confirmation_label());
        mac.update(&self.transcript);
        Ok(mac.verify_slice(&confirmation.mac).is_ok())
    }

    fn confirmation_mac(&self, side: Side) -> [u8; KEY_SIZE] {
        let mut mac = self.hmac();
        mac.update(side.confirmation_label());
        mac.update(&self.transcript);
        mac.finalize().into_bytes().into()
    }

    fn hmac(&self) -> HmacSha256 {
        HmacSha256::new_from_slice(&self.auth_key).expect("SHA-256 HMAC accepts 32-byte keys")
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct Confirm {
    #[serde(with = "serde_bytes")]
    mac: [u8; KEY_SIZE],
}

impl Confirm {
    fn decode(raw: &[u8]) -> Result<Self, AuthError> {
        let mut reader = Cursor::new(raw);
        let confirmation = ciborium::from_reader(&mut reader)
            .map_err(|error| AuthError::InvalidConfirm(error.to_string()))?;

        if usize::try_from(reader.position()).ok() != Some(raw.len()) {
            return Err(AuthError::TrailingConfirmData);
        }

        Ok(confirmation)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VerificationCode(u32);

impl VerificationCode {
    fn from_key(key: &[u8]) -> Self {
        let value = u32::from_be_bytes([key[0], key[1], key[2], key[3]]) % 1_000_000;
        Self(value)
    }
}

impl fmt::Display for VerificationCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:06}", self.0)
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("secure random generation failed: {0}")]
    Random(String),
    #[error("hello encoding failed: {0}")]
    HelloEncoding(String),
    #[error("hello is invalid: {0}")]
    InvalidHello(String),
    #[error("hello contains trailing data")]
    TrailingHelloData,
    #[error("confirmation encoding failed: {0}")]
    ConfirmEncoding(String),
    #[error("confirmation is invalid: {0}")]
    InvalidConfirm(String),
    #[error("confirmation contains trailing data")]
    TrailingConfirmData,
    #[error("protocol version {0} is not supported")]
    UnsupportedVersion(u8),
    #[error("peer reused the local session device ID")]
    DuplicateSessionDeviceId,
    #[error("peer public key does not contribute to a shared secret")]
    NonContributoryPublicKey,
    #[error("session key derivation failed")]
    KeyDerivation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peers_derive_matching_code_and_confirmations() {
        let client = PendingHandshake::start([1; 8]).unwrap();
        let server = PendingHandshake::start([2; 8]).unwrap();
        let client_hello = client.hello_raw().to_vec();
        let server_hello = server.hello_raw().to_vec();

        let client_keys = client.finish(&server_hello, Side::ClientA).unwrap();
        let server_keys = server.finish(&client_hello, Side::ServerB).unwrap();

        assert_eq!(
            client_keys.verification_code(),
            server_keys.verification_code()
        );
        assert_eq!(client_keys.verification_code().to_string().len(), 6);

        let client_confirmation = client_keys.confirmation(Side::ClientA).unwrap();
        let server_confirmation = server_keys.confirmation(Side::ServerB).unwrap();
        assert!(
            server_keys
                .verify_confirmation(Side::ClientA, &client_confirmation)
                .unwrap()
        );
        assert!(
            client_keys
                .verify_confirmation(Side::ServerB, &server_confirmation)
                .unwrap()
        );
    }

    #[test]
    fn tampered_hello_fails_confirmation() {
        let client = PendingHandshake::start([3; 8]).unwrap();
        let server = PendingHandshake::start([4; 8]).unwrap();
        let attacker = PendingHandshake::start([5; 8]).unwrap();
        let client_hello = client.hello_raw().to_vec();
        let attacker_hello = attacker.hello_raw().to_vec();

        let client_keys = client.finish(&attacker_hello, Side::ClientA).unwrap();
        let server_keys = server.finish(&client_hello, Side::ServerB).unwrap();

        let client_confirmation = client_keys.confirmation(Side::ClientA).unwrap();

        assert!(
            !server_keys
                .verify_confirmation(Side::ClientA, &client_confirmation)
                .unwrap()
        );
    }

    #[test]
    fn confirmation_is_bound_to_side() {
        let client = PendingHandshake::start([6; 8]).unwrap();
        let server = PendingHandshake::start([7; 8]).unwrap();
        let client_hello = client.hello_raw().to_vec();
        let server_hello = server.hello_raw().to_vec();
        let client_keys = client.finish(&server_hello, Side::ClientA).unwrap();
        let server_keys = server.finish(&client_hello, Side::ServerB).unwrap();
        let confirmation = client_keys.confirmation(Side::ClientA).unwrap();

        assert!(
            !server_keys
                .verify_confirmation(Side::ServerB, &confirmation)
                .unwrap()
        );
    }

    #[test]
    fn hello_with_trailing_data_is_rejected() {
        let handshake = PendingHandshake::start([8; 8]).unwrap();
        let mut hello = handshake.hello_raw().to_vec();
        hello.push(0);

        assert!(matches!(
            Hello::decode(&hello),
            Err(AuthError::TrailingHelloData)
        ));
    }
}
