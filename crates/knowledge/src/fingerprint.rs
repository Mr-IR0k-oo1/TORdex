use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A content-based fingerprint uniquely identifying a knowledge record.
///
/// Once computed, the fingerprint is **immutable** and can be used for
/// deduplication, identity resolution, and integrity verification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fingerprint {
    /// SHA-256 digest of the canonical JSON representation.
    Sha256([u8; 32]),
    /// 64-bit SimHash fingerprint for similarity comparisons.
    SimHash(u64),
    /// 128-bit content hash using BLAKE2 or other fast hash.
    ContentHash([u8; 16]),
}

impl Fingerprint {
    /// Compute a SHA-256 fingerprint from serializable content.
    #[must_use]
    pub fn sha256(content: &serde_json::Value) -> Self {
        let canonical = serde_json::to_vec(content).unwrap_or_default();
        let hash = Sha256::digest(&canonical);
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&hash);
        Self::Sha256(arr)
    }

    /// Compute a 64-bit SimHash fingerprint from a token iterator.
    ///
    /// Each token contributes its hash to the fingerprint, suitable for
    /// detecting near-duplicate content.
    #[must_use]
    pub fn simhash<'a>(tokens: impl Iterator<Item = &'a [u8]>) -> Self {
        let mut v = [0i64; 64];
        for token in tokens {
            let h = sha2token(token);
            for i in 0..64 {
                let bit = (h >> i) & 1;
                if bit == 1 {
                    v[i] += 1;
                } else {
                    v[i] -= 1;
                }
            }
        }
        let mut fp = 0u64;
        for i in 0..64 {
            if v[i] > 0 {
                fp |= 1 << i;
            }
        }
        Self::SimHash(fp)
    }

    /// Compute a fast content hash (truncated SHA-256) from raw bytes.
    #[must_use]
    pub fn content_hash(data: &[u8]) -> Self {
        let hash = Sha256::digest(data);
        let mut arr = [0u8; 16];
        arr.copy_from_slice(&hash[..16]);
        Self::ContentHash(arr)
    }

    /// Return the hex-encoded string representation.
    #[must_use]
    pub fn hex(&self) -> String {
        match self {
            Self::Sha256(b) => hex::encode(b),
            Self::SimHash(h) => format!("{:016x}", h),
            Self::ContentHash(b) => hex::encode(b),
        }
    }

    /// Return the Hamming distance between two fingerprints.
    ///
    /// Only meaningful when both fingerprints are of the same variant.
    #[must_use]
    pub fn hamming_distance(&self, other: &Self) -> Option<u32> {
        match (self, other) {
            (Self::Sha256(a), Self::Sha256(b)) => {
                Some(a.iter().zip(b.iter()).map(|(x, y)| (x ^ y).count_ones()).sum())
            }
            (Self::SimHash(a), Self::SimHash(b)) => Some((a ^ b).count_ones()),
            (Self::ContentHash(a), Self::ContentHash(b)) => {
                Some(a.iter().zip(b.iter()).map(|(x, y)| (x ^ y).count_ones()).sum())
            }
            _ => None,
        }
    }
}

fn sha2token(data: &[u8]) -> u64 {
    let hash = Sha256::digest(data);
    u64::from_ne_bytes(hash[..8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_fingerprint_is_deterministic() {
        let v = serde_json::json!({"a": 1, "b": 2});
        let a = Fingerprint::sha256(&v);
        let b = Fingerprint::sha256(&v);
        assert_eq!(a, b);
    }

    #[test]
    fn different_content_different_fingerprint() {
        let a = Fingerprint::sha256(&serde_json::json!({"a": 1}));
        let b = Fingerprint::sha256(&serde_json::json!({"a": 2}));
        assert_ne!(a, b);
    }

    #[test]
    fn simhash_produces_64bit() {
        let fp = Fingerprint::simhash(b"hello world\nfoo bar".split(|&b| b == b' '));
        assert!(matches!(fp, Fingerprint::SimHash(_)));
        let hex = fp.hex();
        assert_eq!(hex.len(), 16);
    }

    #[test]
    fn content_hash_is_stable() {
        let a = Fingerprint::content_hash(b"hello");
        let b = Fingerprint::content_hash(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn hamming_distance_same() {
        let a = Fingerprint::SimHash(0xFFFF);
        let b = Fingerprint::SimHash(0xFFFF);
        assert_eq!(a.hamming_distance(&b), Some(0));
    }

    #[test]
    fn hamming_distance_max() {
        let a = Fingerprint::SimHash(0);
        let b = Fingerprint::SimHash(!0u64);
        assert_eq!(a.hamming_distance(&b), Some(64));
    }

    #[test]
    fn hamming_distance_different_variants_returns_none() {
        let a = Fingerprint::Sha256([0; 32]);
        let b = Fingerprint::SimHash(0);
        assert!(a.hamming_distance(&b).is_none());
    }
}
