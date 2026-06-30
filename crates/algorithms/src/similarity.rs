use std::collections::{BTreeSet, HashMap, HashSet};
use sha2::{Digest, Sha256};

/// Jaccard similarity coefficient between two sets.
pub fn jaccard_similarity<T: std::hash::Hash + Eq>(a: &HashSet<T>, b: &HashSet<T>) -> f64 {
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        return 1.0;
    }
    intersection as f64 / union as f64
}

/// Cosine similarity between two vectors.
pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom == 0.0 {
        return 0.0;
    }
    dot / denom
}

/// Cosine similarity for term frequency vectors.
pub fn cosine_similarity_text(a: &str, b: &str) -> f64 {
    let tf_a = term_frequencies(a);
    let tf_b = term_frequencies(b);
    let mut all_terms: HashSet<&str> = tf_a.keys().chain(tf_b.keys()).copied().collect();
    if all_terms.is_empty() {
        return 0.0;
    }
    let mut vec_a = Vec::new();
    let mut vec_b = Vec::new();
    let mut sorted: Vec<&str> = all_terms.drain().collect();
    sorted.sort();
    for term in &sorted {
        vec_a.push(*tf_a.get(term).unwrap_or(&0.0));
        vec_b.push(*tf_b.get(term).unwrap_or(&0.0));
    }
    cosine_similarity(&vec_a, &vec_b)
}

fn term_frequencies(text: &str) -> HashMap<&str, f64> {
    let mut freq = HashMap::new();
    let total = text.split_whitespace().count() as f64;
    if total == 0.0 {
        return freq;
    }
    for term in text.split_whitespace() {
        *freq.entry(term).or_insert(0.0) += 1.0;
    }
    for val in freq.values_mut() {
        *val /= total;
    }
    freq
}

/// MinHash for estimating Jaccard similarity between sets.
#[derive(Clone, Debug)]
pub struct MinHash {
    hash_functions: Vec<u64>,
    signatures: Vec<Vec<u64>>,
}

impl MinHash {
    pub fn new(num_hashes: usize) -> Self {
        let hash_functions: Vec<u64> = (0..num_hashes as u64)
            .map(|i| 1_000_003u64.wrapping_mul(i + 1).wrapping_add(1_000_009))
            .collect();
        MinHash {
            hash_functions,
            signatures: Vec::new(),
        }
    }

    pub fn signature(&self, set: &BTreeSet<String>) -> Vec<u64> {
        let mut sig = vec![u64::MAX; self.hash_functions.len()];
        for element in set {
            let hash = hash_string(element);
            for (i, &coeff) in self.hash_functions.iter().enumerate() {
                let h = hash.wrapping_mul(coeff);
                if h < sig[i] {
                    sig[i] = h;
                }
            }
        }
        sig
    }

    pub fn add_set(&mut self, set: BTreeSet<String>) -> usize {
        let sig = self.signature(&set);
        self.signatures.push(sig);
        self.signatures.len() - 1
    }

    pub fn estimate_similarity(&self, i: usize, j: usize) -> f64 {
        if i >= self.signatures.len() || j >= self.signatures.len() {
            return 0.0;
        }
        let matches = self.signatures[i]
            .iter()
            .zip(self.signatures[j].iter())
            .filter(|(a, b)| a == b)
            .count();
        matches as f64 / self.signatures[i].len() as f64
    }
}

fn hash_string(s: &str) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let result = hasher.finalize();
    u64::from_le_bytes(result[..8].try_into().unwrap())
}

/// SimHash for near-duplicate detection.
#[derive(Clone, Debug)]
pub struct SimHash;

impl SimHash {
    pub fn hash(text: &str) -> u64 {
        let mut v = vec![0i64; 64];
        let terms: Vec<&str> = text.split_whitespace().collect();
        if terms.is_empty() {
            return 0;
        }
        let total = terms.len() as i64;

        for term in &terms {
            let term_hash = hash_string(term);
            for i in 0..64 {
                let bit = (term_hash >> i) & 1;
                if bit == 1 {
                    v[i] += total;
                } else {
                    v[i] -= total;
                }
            }
        }

        let mut fingerprint = 0u64;
        for i in 0..64 {
            if v[i] > 0 {
                fingerprint |= 1 << i;
            }
        }
        fingerprint
    }

    pub fn similarity(a: u64, b: u64) -> f64 {
        let diff = (a ^ b).count_ones() as f64;
        1.0 - (diff / 64.0)
    }

    pub fn are_near_duplicates(a: u64, b: u64, threshold: f64) -> bool {
        Self::similarity(a, b) >= threshold
    }
}

/// Locality-Sensitive Hashing using MinHash signatures.
#[derive(Clone, Debug)]
pub struct LSH {
    minhash: MinHash,
    bands: usize,
    rows: usize,
    buckets: HashMap<u64, Vec<usize>>,
}

impl LSH {
    pub fn new(num_hashes: usize, bands: usize) -> Self {
        let rows = num_hashes / bands;
        LSH {
            minhash: MinHash::new(num_hashes),
            bands,
            rows,
            buckets: HashMap::new(),
        }
    }

    pub fn insert(&mut self, set: BTreeSet<String>) -> usize {
        let idx = self.minhash.add_set(set);
        let sig = &self.minhash.signatures[idx];
        for b in 0..self.bands {
            let start = b * self.rows;
            let end = (start + self.rows).min(sig.len());
            let band_hash = self.hash_band(&sig[start..end]);
            self.buckets.entry(band_hash).or_default().push(idx);
        }
        idx
    }

    pub fn query(&self, set: &BTreeSet<String>) -> HashSet<usize> {
        let sig = self.minhash.signature(set);
        let mut candidates = HashSet::new();
        for b in 0..self.bands {
            let start = b * self.rows;
            let end = (start + self.rows).min(sig.len());
            let band_hash = self.hash_band(&sig[start..end]);
            if let Some(bucket) = self.buckets.get(&band_hash) {
                for &idx in bucket {
                    candidates.insert(idx);
                }
            }
        }
        candidates
    }

    fn hash_band(&self, band: &[u64]) -> u64 {
        let mut hasher = Sha256::new();
        for &v in band {
            hasher.update(v.to_le_bytes());
        }
        let result = hasher.finalize();
        u64::from_le_bytes(result[..8].try_into().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jaccard_identical_sets() {
        let mut a = HashSet::new();
        a.insert(1);
        a.insert(2);
        let b = a.clone();
        assert!((jaccard_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn jaccard_disjoint_sets() {
        let mut a = HashSet::new();
        a.insert(1);
        let mut b = HashSet::new();
        b.insert(2);
        assert!((jaccard_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn cosine_identical_vectors() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_orthogonal_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn cosine_text_similarity() {
        let sim = cosine_similarity_text("hello world", "hello world");
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn minhash_identical_sets() {
        let mut mh = MinHash::new(256);
        let s1 = "this is a test document with several words".to_string();
        let s2 = "this is a test document with several words".to_string();
        let set1: BTreeSet<String> = s1.split_whitespace().map(|s| s.to_string()).collect();
        let set2: BTreeSet<String> = s2.split_whitespace().map(|s| s.to_string()).collect();
        let i = mh.add_set(set1);
        let j = mh.add_set(set2);
        let sim = mh.estimate_similarity(i, j);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn simhash_identical_text() {
        let h1 = SimHash::hash("the quick brown fox");
        let h2 = SimHash::hash("the quick brown fox");
        assert!((SimHash::similarity(h1, h2) - 1.0).abs() < 0.001);
    }

    #[test]
    fn simhash_near_duplicate() {
        let h1 = SimHash::hash("the quick brown fox jumps over the lazy dog");
        let h2 = SimHash::hash("the quick brown fox jumps over the lazy cat");
        assert!(SimHash::are_near_duplicates(h1, h2, 0.5));
    }

    #[test]
    fn lsh_basic() {
        // Use 128 bands with 1 row each for maximal sensitivity
        let mut lsh = LSH::new(128, 128);
        let mut set1 = BTreeSet::new();
        set1.insert("a".to_string());
        set1.insert("b".to_string());
        let _ = lsh.insert(set1);

        let mut set2 = BTreeSet::new();
        set2.insert("a".to_string());
        set2.insert("b".to_string());
        set2.insert("c".to_string());
        let candidate = lsh.query(&set2);
        assert!(!candidate.is_empty());
    }
}
