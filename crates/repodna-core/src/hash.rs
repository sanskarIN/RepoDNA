//! Deterministic hashing helpers.
//!
//! RepoDNA uses SHA-256 for content hashes, stable finding identifiers, and the repository
//! DNA hash because it is well specified, portable, and collision resistant. None of these
//! values are security primitives: they identify data, they do not authenticate it.

use sha2::{Digest, Sha256};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// Encodes bytes as lowercase hexadecimal.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

/// Returns the SHA-256 digest of `data`.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..]);
    out
}

/// Returns the lowercase hexadecimal SHA-256 digest of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    to_hex(&sha256(data))
}

/// Returns the first 16 hexadecimal characters (64 bits) of the SHA-256 digest of `data`.
///
/// Used for compact file content hashes in artifacts, where the full digest would add size
/// without practical benefit.
pub fn short_hash(data: &[u8]) -> String {
    to_hex(&sha256(data)[..8])
}

/// Builds a short, stable identifier from ordered string parts.
///
/// Parts are separated by the ASCII unit separator so `["ab", "c"]` and `["a", "bc"]`
/// produce different identifiers. The result has 12 hexadecimal characters.
pub fn stable_id<S: AsRef<str>>(parts: &[S]) -> String {
    let mut hasher = Sha256::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            hasher.update([0x1f]);
        }
        hasher.update(part.as_ref().as_bytes());
    }
    to_hex(&hasher.finalize()[..6])
}

/// Computes HMAC-SHA256 (RFC 2104) of `message` under `key`.
///
/// Secret-candidate fingerprints are keyed with a digest of the scanned content so a shared
/// report cannot be used to confirm guesses of a redacted value.
pub fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK_SIZE: usize = 64;
    let mut block = [0u8; BLOCK_SIZE];
    if key.len() > BLOCK_SIZE {
        block[..32].copy_from_slice(&sha256(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }

    let mut inner = Sha256::new();
    inner.update(block.map(|b| b ^ 0x36));
    inner.update(message);
    let inner_digest = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(block.map(|b| b ^ 0x5c));
    outer.update(&inner_digest[..]);
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer.finalize()[..]);
    out
}

/// Incrementally hashes a sequence of byte strings with SHA-256.
#[derive(Clone, Default)]
pub struct StableHasher {
    inner: Sha256,
}

impl StableHasher {
    /// Creates an empty hasher.
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds a length-prefixed field so concatenated inputs cannot collide.
    pub fn field(&mut self, bytes: &[u8]) -> &mut Self {
        self.inner.update((bytes.len() as u64).to_le_bytes());
        self.inner.update(bytes);
        self
    }

    /// Feeds a length-prefixed UTF-8 string.
    pub fn str_field(&mut self, value: &str) -> &mut Self {
        self.field(value.as_bytes())
    }

    /// Returns the digest as lowercase hexadecimal.
    pub fn finish_hex(&self) -> String {
        to_hex(&self.inner.clone().finalize()[..])
    }

    /// Returns the raw 32-byte digest.
    pub fn finish(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(&self.inner.clone().finalize()[..]);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(short_hash(b"abc"), "ba7816bf8f01cfea");
    }

    #[test]
    fn hmac_matches_rfc4231_test_cases() {
        // Test case 1.
        let key = [0x0b; 20];
        assert_eq!(
            to_hex(&hmac_sha256(&key, b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // Test case 2.
        assert_eq!(
            to_hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Test case 6: a key longer than the block size is hashed first.
        let long_key = [0xaa; 131];
        assert_eq!(
            to_hex(&hmac_sha256(
                &long_key,
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            )),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn stable_ids_separate_parts() {
        assert_eq!(stable_id(&["a", "b"]), stable_id(&["a", "b"]));
        assert_ne!(stable_id(&["ab", "c"]), stable_id(&["a", "bc"]));
        assert_eq!(stable_id(&["x"]).len(), 12);
    }

    #[test]
    fn stable_hasher_is_length_prefixed() {
        let mut first = StableHasher::new();
        first.str_field("ab").str_field("c");
        let mut second = StableHasher::new();
        second.str_field("a").str_field("bc");
        assert_ne!(first.finish_hex(), second.finish_hex());
        assert_eq!(first.finish_hex().len(), 64);
    }
}
