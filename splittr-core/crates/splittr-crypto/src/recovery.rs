//! BIP39 recovery phrase for the 32-byte identity seed (#34).
//!
//! The phrase is the server-free backup/restore path: 32 bytes of entropy ⇄ a
//! 24-word mnemonic with a checksum. Restoring the phrase re-derives the same
//! identity (and therefore the same user id).

use bip39::Mnemonic;

/// A 24-word recovery phrase encoding the identity seed.
pub fn recovery_phrase(seed: &[u8; 32]) -> String {
    Mnemonic::from_entropy(seed)
        .expect("32 bytes is valid BIP39 entropy")
        .to_string()
}

/// Parse a recovery phrase back into the 32-byte seed. Returns `None` if the
/// phrase is malformed, has a bad checksum, or is not 256-bit.
pub fn seed_from_phrase(phrase: &str) -> Option<[u8; 32]> {
    let mnemonic = Mnemonic::parse(phrase.trim()).ok()?;
    let (entropy, len) = mnemonic.to_entropy_array();
    if len != 32 {
        return None;
    }
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&entropy[..32]);
    Some(seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_round_trips_the_seed() {
        let seed = [42u8; 32];
        let phrase = recovery_phrase(&seed);
        assert_eq!(phrase.split_whitespace().count(), 24);
        assert_eq!(seed_from_phrase(&phrase), Some(seed));
    }

    #[test]
    fn distinct_seeds_give_distinct_phrases() {
        assert_ne!(recovery_phrase(&[1u8; 32]), recovery_phrase(&[2u8; 32]));
    }

    #[test]
    fn rejects_garbage_and_tampered_phrases() {
        assert_eq!(seed_from_phrase("not a real phrase"), None);
        // Swap two words → checksum fails.
        let phrase = recovery_phrase(&[7u8; 32]);
        let mut words: Vec<&str> = phrase.split_whitespace().collect();
        words.swap(0, 1);
        assert_eq!(seed_from_phrase(&words.join(" ")), None);
    }
}
