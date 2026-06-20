//! Cross-device enrolment transcript + short authentication string (#35).
//!
//! Linking a new device to an existing identity happens over an authenticated
//! channel (QR + a one-time challenge). The channel itself can be MITM'd, so
//! both devices independently derive a **short authentication string** (SAS)
//! from the full pairing transcript and the users compare it out-of-band. A
//! man-in-the-middle that swapped either device key (or the challenge) produces
//! a different SAS on each side, so the mismatch is visible.
//!
//! The derivation is a pure function of the transcript — both sides hold the
//! same four values in the same roles, so it is unit-testable without any
//! transport. The transport, QR encoding and camera live in the shell.

use blake3::Hasher;

use crate::keys::PublicKey;

/// Everything both devices agree on before authorizing the new device. The SAS
/// binds all of it, so tampering with any field changes the displayed code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PairingTranscript {
    /// The identity (root public key) the new device is joining.
    pub identity: PublicKey,
    /// The existing (primary) device's public key.
    pub primary_device: PublicKey,
    /// The joining device's public key.
    pub new_device: PublicKey,
    /// A one-time challenge carried over the pairing channel (QR).
    pub challenge: [u8; 32],
}

/// Number of decimal digits in the SAS. Six digits (~20 bits) is the usual
/// trade-off between shoulder-surfing comparison effort and MITM resistance.
const SAS_DIGITS: u32 = 6;

impl PairingTranscript {
    /// The shared short authentication string, e.g. `"472 905"`. Both devices
    /// compute the same value from the same transcript; a tampered transcript
    /// (swapped key or challenge) yields a different one.
    pub fn short_auth_string(&self) -> String {
        let mut hasher = Hasher::new();
        // Domain separation so this hash can never collide with op-id hashing.
        hasher.update(b"splittr-pairing-sas-v1");
        hasher.update(&self.identity.0);
        hasher.update(&self.primary_device.0);
        hasher.update(&self.new_device.0);
        hasher.update(&self.challenge);
        let digest = hasher.finalize();

        // Fold the first 8 bytes into a number, then take the low SAS_DIGITS.
        let bytes: [u8; 8] = digest.as_bytes()[..8].try_into().expect("8 bytes");
        let modulus = 10u64.pow(SAS_DIGITS);
        let code = u64::from_be_bytes(bytes) % modulus;
        let half = SAS_DIGITS / 2;
        format!(
            "{:0width$} {:0width$}",
            code / 10u64.pow(half),
            code % 10u64.pow(half),
            width = half as usize,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transcript() -> PairingTranscript {
        PairingTranscript {
            identity: PublicKey([1u8; 32]),
            primary_device: PublicKey([2u8; 32]),
            new_device: PublicKey([3u8; 32]),
            challenge: [4u8; 32],
        }
    }

    #[test]
    fn both_sides_agree() {
        // The same transcript on either device yields the same SAS.
        assert_eq!(
            transcript().short_auth_string(),
            transcript().short_auth_string()
        );
    }

    #[test]
    fn formats_as_two_groups_of_three_digits() {
        let sas = transcript().short_auth_string();
        let parts: Vec<&str> = sas.split(' ').collect();
        assert_eq!(parts.len(), 2);
        assert!(parts
            .iter()
            .all(|p| p.len() == 3 && p.chars().all(|c| c.is_ascii_digit())));
    }

    #[test]
    fn tampered_new_device_changes_sas() {
        let honest = transcript().short_auth_string();
        let mut mitm = transcript();
        mitm.new_device = PublicKey([99u8; 32]); // attacker substitutes its key
        assert_ne!(honest, mitm.short_auth_string());
    }

    #[test]
    fn tampered_challenge_changes_sas() {
        let honest = transcript().short_auth_string();
        let mut mitm = transcript();
        mitm.challenge = [7u8; 32];
        assert_ne!(honest, mitm.short_auth_string());
    }

    #[test]
    fn swapping_device_roles_changes_sas() {
        // primary/new are distinct roles; swapping them is a different transcript.
        let normal = transcript().short_auth_string();
        let mut swapped = transcript();
        std::mem::swap(&mut swapped.primary_device, &mut swapped.new_device);
        assert_ne!(normal, swapped.short_auth_string());
    }
}
