use std::collections::BTreeMap;
use sha2::{Digest, Sha256};

/// A Merkle Tree for data integrity verification.
#[derive(Clone, Debug)]
pub struct MerkleTree {
    root: Option<[u8; 32]>,
    leaves: Vec<[u8; 32]>,
    levels: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    pub fn new() -> Self {
        MerkleTree {
            root: None,
            leaves: Vec::new(),
            levels: Vec::new(),
        }
    }

    pub fn from_data(data: &[&[u8]]) -> Self {
        let mut tree = MerkleTree::new();
        for chunk in data {
            tree.add_leaf(chunk);
        }
        tree.build();
        tree
    }

    pub fn add_leaf(&mut self, data: &[u8]) {
        let hash = hash_bytes(data);
        self.leaves.push(hash);
    }

    pub fn build(&mut self) {
        if self.leaves.is_empty() {
            self.root = None;
            self.levels.clear();
            return;
        }
        self.levels.clear();
        let mut current_level = self.leaves.clone();
        self.levels.push(current_level.clone());
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            for chunk in current_level.chunks(2) {
                if chunk.len() == 2 {
                    let combined = [chunk[0].as_slice(), chunk[1].as_slice()].concat();
                    next_level.push(hash_bytes(&combined));
                } else {
                    next_level.push(chunk[0]);
                }
            }
            self.levels.push(next_level.clone());
            current_level = next_level;
        }
        self.root = Some(current_level[0]);
    }

    pub fn root(&self) -> Option<[u8; 32]> {
        self.root
    }

    pub fn root_hex(&self) -> Option<String> {
        self.root.map(|r| hex::encode(r))
    }

    pub fn verify(&self, data: &[u8], proof: &[([u8; 32], bool)]) -> bool {
        let mut hash = hash_bytes(data);
        for &(sibling, is_right) in proof {
            let combined = if is_right {
                [hash.as_slice(), sibling.as_slice()].concat()
            } else {
                [sibling.as_slice(), hash.as_slice()].concat()
            };
            hash = hash_bytes(&combined);
        }
        Some(hash) == self.root
    }

    pub fn generate_proof(&self, leaf_index: usize) -> Option<Vec<([u8; 32], bool)>> {
        if leaf_index >= self.leaves.len() || self.levels.is_empty() {
            return None;
        }
        let mut proof = Vec::new();
        let mut idx = leaf_index;
        for level in &self.levels[..self.levels.len() - 1] {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            if sibling_idx < level.len() {
                proof.push((level[sibling_idx], idx % 2 == 0));
            }
            idx /= 2;
        }
        Some(proof)
    }

    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

fn hash_bytes(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&result);
    arr
}

/// A Bloom Filter for probabilistic set membership.
#[derive(Clone, Debug)]
pub struct BloomFilter {
    bits: Vec<u64>,
    num_hashes: usize,
    size: usize,
    inserted: u64,
}

impl BloomFilter {
    pub fn new(size: usize, num_hashes: usize) -> Self {
        let word_count = (size + 63) / 64;
        BloomFilter {
            bits: vec![0u64; word_count],
            num_hashes,
            size,
            inserted: 0,
        }
    }

    pub fn with_false_positive_rate(expected_items: usize, fp_rate: f64) -> Self {
        let ln2 = std::f64::consts::LN_2;
        let size = (-(expected_items as f64) * fp_rate.ln() / (ln2 * ln2)).ceil() as usize;
        let num_hashes = ((size as f64 / expected_items as f64) * ln2).ceil() as usize;
        Self::new(size.max(1), num_hashes.max(1))
    }

    pub fn insert(&mut self, item: &[u8]) {
        for i in 0..self.num_hashes {
            let bit = self.hash(item, i) % self.size as u64;
            let word = (bit / 64) as usize;
            let offset = (bit % 64) as usize;
            if word < self.bits.len() {
                self.bits[word] |= 1u64 << offset;
            }
        }
        self.inserted += 1;
    }

    pub fn contains(&self, item: &[u8]) -> bool {
        for i in 0..self.num_hashes {
            let bit = self.hash(item, i) % self.size as u64;
            let word = (bit / 64) as usize;
            let offset = (bit % 64) as usize;
            if word >= self.bits.len() {
                return false;
            }
            if self.bits[word] & (1u64 << offset) == 0 {
                return false;
            }
        }
        true
    }

    pub fn insert_str(&mut self, item: &str) {
        self.insert(item.as_bytes());
    }

    pub fn contains_str(&self, item: &str) -> bool {
        self.contains(item.as_bytes())
    }

    pub fn false_positive_rate(&self) -> f64 {
        let m = self.size as f64;
        let k = self.num_hashes as f64;
        let n = self.inserted as f64;
        if m == 0.0 {
            return 1.0;
        }
        (1.0 - (-(k * n / m)).exp()).powf(k)
    }

    pub fn clear(&mut self) {
        for word in &mut self.bits {
            *word = 0;
        }
        self.inserted = 0;
    }

    fn hash(&self, data: &[u8], seed: usize) -> u64 {
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.update(seed.to_le_bytes());
        let result = hasher.finalize();
        u64::from_le_bytes(result[..8].try_into().unwrap())
    }
}

/// HyperLogLog for cardinality estimation.
#[derive(Clone, Debug)]
pub struct HyperLogLog {
    registers: Vec<u8>,
    precision: u8,
}

impl HyperLogLog {
    pub fn new(precision: u8) -> Self {
        let size = 1 << precision;
        HyperLogLog {
            registers: vec![0u8; size],
            precision,
        }
    }

    pub fn insert(&mut self, data: &[u8]) {
        let hash = hash_bytes(data);
        let full_hash = u64::from_le_bytes(hash[..8].try_into().unwrap());
        let index = (full_hash >> (64 - self.precision)) as usize;
        let remaining = full_hash << self.precision;
        let rank = remaining.leading_zeros() as u8 + 1;

        if rank > self.registers[index] {
            self.registers[index] = rank;
        }
    }

    pub fn insert_str(&mut self, item: &str) {
        self.insert(item.as_bytes());
    }

    pub fn estimate(&self) -> f64 {
        let m = self.registers.len() as f64;
        let sum: f64 = self.registers.iter().map(|&r| 2.0f64.powi(-(r as i32))).sum();
        let estimate = (0.7213 / (1.0 + 1.079 / m)) * m * m / sum;

        let small_range = 5.0 * m;
        if estimate <= small_range {
            let zeros = self.registers.iter().filter(|&&r| r == 0).count() as f64;
            if zeros > 0.0 {
                return m * (m / zeros).ln();
            }
        }
        estimate
    }

    pub fn merge(&mut self, other: &HyperLogLog) {
        if self.registers.len() != other.registers.len() {
            return;
        }
        for (a, &b) in self.registers.iter_mut().zip(other.registers.iter()) {
            if b > *a {
                *a = b;
            }
        }
    }
}

/// A B+ Tree for sorted key-value storage.
#[derive(Clone, Debug)]
pub struct BPlusTree<K: Ord + Clone, V: Clone> {
    keys: Vec<K>,
    values: Vec<V>,
    #[allow(dead_code)]
    order: usize,
}

impl<K: Ord + Clone, V: Clone> BPlusTree<K, V> {
    pub fn new(order: usize) -> Self {
        BPlusTree {
            keys: Vec::new(),
            values: Vec::new(),
            order,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let pos = self.keys.binary_search(&key).unwrap_or_else(|e| e);
        self.keys.insert(pos, key);
        self.values.insert(pos, value);
    }

    pub fn search(&self, key: &K) -> Option<V> {
        self.keys
            .binary_search(key)
            .ok()
            .map(|i| self.values[i].clone())
    }

    pub fn range_scan(&self, start: &K, end: &K) -> Vec<(K, V)> {
        let start_pos = self.keys.binary_search(start).unwrap_or_else(|e| e);
        let end_pos = self
            .keys
            .binary_search(end)
            .unwrap_or_else(|e| e.saturating_sub(1));
        let mut results = Vec::new();
        for i in start_pos..=end_pos.min(self.keys.len().saturating_sub(1)) {
            results.push((self.keys[i].clone(), self.values[i].clone()));
        }
        results
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}

/// An LSM Tree (Log-Structured Merge Tree) for high-write-throughput storage.
#[derive(Clone, Debug)]
pub struct LSMTree<K: Ord + Clone + std::fmt::Debug, V: Clone> {
    memtable: BTreeMap<K, V>,
    sstables: Vec<BTreeMap<K, V>>,
    max_memtable_size: usize,
    compaction_threshold: usize,
}

impl<K: Ord + Clone + std::fmt::Debug, V: Clone> LSMTree<K, V> {
    pub fn new(max_memtable_size: usize) -> Self {
        LSMTree {
            memtable: BTreeMap::new(),
            sstables: Vec::new(),
            max_memtable_size,
            compaction_threshold: 4,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.memtable.insert(key, value);
        if self.memtable.len() >= self.max_memtable_size {
            self.flush();
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        if let Some(value) = self.memtable.get(key) {
            return Some(value.clone());
        }
        for sstable in self.sstables.iter().rev() {
            if let Some(value) = sstable.get(key) {
                return Some(value.clone());
            }
        }
        None
    }

    pub fn contains_key(&self, key: &K) -> bool {
        self.memtable.contains_key(key)
            || self.sstables.iter().rev().any(|s| s.contains_key(key))
    }

    pub fn flush(&mut self) {
        if self.memtable.is_empty() {
            return;
        }
        let frozen = std::mem::take(&mut self.memtable);
        self.sstables.push(frozen);
        if self.sstables.len() >= self.compaction_threshold {
            self.compact();
        }
    }

    fn compact(&mut self) {
        let mut merged = BTreeMap::new();
        for sstable in self.sstables.drain(..) {
            for (k, v) in sstable {
                merged.insert(k, v);
            }
        }
        if merged.len() > self.max_memtable_size * 2 {
            let mut new_sstables = Vec::new();
            let entries: Vec<(K, V)> = merged.into_iter().collect();
            for chunk in entries.chunks(self.max_memtable_size) {
                let mut sstable = BTreeMap::new();
                for (k, v) in chunk {
                    sstable.insert(k.clone(), v.clone());
                }
                new_sstables.push(sstable);
            }
            self.sstables = new_sstables;
        } else {
            self.sstables.push(merged);
        }
    }

    pub fn memtable_size(&self) -> usize {
        self.memtable.len()
    }

    pub fn sstable_count(&self) -> usize {
        self.sstables.len()
    }

    pub fn total_entries(&self) -> usize {
        let mut total = self.memtable.len();
        for sstable in &self.sstables {
            total += sstable.len();
        }
        total
    }

    pub fn range_scan(&self, start: &K, end: &K) -> Vec<(K, V)> {
        let mut results = BTreeMap::new();
        for (k, v) in self.memtable.range(start..=end) {
            results.insert(k.clone(), v.clone());
        }
        for sstable in self.sstables.iter().rev() {
            for (k, v) in sstable.range(start..=end) {
                results.entry(k.clone()).or_insert_with(|| v.clone());
            }
        }
        results.into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merkle_tree_basic() {
        let tree = MerkleTree::from_data(&[b"a", b"b", b"c", b"d"]);
        assert!(tree.root().is_some());
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn merkle_tree_verify_proof() {
        let tree = MerkleTree::from_data(&[b"data1", b"data2", b"data3", b"data4"]);
        let proof = tree.generate_proof(0).unwrap();
        assert!(tree.verify(b"data1", &proof));
        assert!(!tree.verify(b"wrong", &proof));
    }

    #[test]
    fn merkle_tree_single_leaf() {
        let tree = MerkleTree::from_data(&[b"only"]);
        assert!(tree.root().is_some());
    }

    #[test]
    fn bloom_filter_basic() {
        let mut bf = BloomFilter::new(1000, 3);
        bf.insert_str("hello");
        bf.insert_str("world");
        assert!(bf.contains_str("hello"));
        assert!(bf.contains_str("world"));
        assert!(!bf.contains_str("missing"));
    }

    #[test]
    fn bloom_filter_fp_rate() {
        let mut bf = BloomFilter::with_false_positive_rate(100, 0.01);
        for i in 0..100 {
            bf.insert_str(&format!("item{i}"));
        }
        let rate = bf.false_positive_rate();
        assert!(rate < 0.05);
    }

    #[test]
    fn hyperloglog_estimate() {
        let mut hll = HyperLogLog::new(10);
        for i in 0..5000 {
            hll.insert_str(&format!("element{i}"));
        }
        let est = hll.estimate();
        let error = (est - 5000.0).abs() / 5000.0;
        assert!(error < 0.1);
    }

    #[test]
    fn hyperloglog_merge() {
        let mut hll1 = HyperLogLog::new(10);
        let mut hll2 = HyperLogLog::new(10);
        for i in 0..1000 {
            hll1.insert_str(&format!("a{i}"));
        }
        for i in 0..1000 {
            hll2.insert_str(&format!("b{i}"));
        }
        hll1.merge(&hll2);
        let est = hll1.estimate();
        assert!(est > 1500.0 && est < 2500.0);
    }

    #[test]
    fn bplus_tree_insert_search() {
        let mut tree = BPlusTree::new(4);
        tree.insert("key1", "value1");
        tree.insert("key2", "value2");
        assert_eq!(tree.search(&"key1"), Some("value1"));
        assert_eq!(tree.search(&"key3"), None);
    }

    #[test]
    fn bplus_tree_range_scan() {
        let mut tree = BPlusTree::new(4);
        tree.insert("a", 1);
        tree.insert("b", 2);
        tree.insert("c", 3);
        tree.insert("d", 4);
        let range = tree.range_scan(&"b", &"c");
        assert_eq!(range.len(), 2);
    }

    #[test]
    fn lsm_tree_basic() {
        let mut lsm = LSMTree::new(5);
        lsm.insert("a", 1);
        lsm.insert("b", 2);
        assert_eq!(lsm.get(&"a"), Some(1));
        assert_eq!(lsm.get(&"c"), None);
    }

    #[test]
    fn lsm_tree_flush_and_compact() {
        let mut lsm = LSMTree::new(2);
        lsm.insert("a", 1);
        lsm.insert("b", 2);
        lsm.insert("c", 3);
        assert!(lsm.sstable_count() >= 1);
        assert_eq!(lsm.get(&"a"), Some(1));
        assert_eq!(lsm.get(&"c"), Some(3));
    }

    #[test]
    fn lsm_tree_range_scan() {
        let mut lsm = LSMTree::new(10);
        lsm.insert("a", 1);
        lsm.insert("b", 2);
        lsm.insert("c", 3);
        lsm.insert("d", 4);
        let range = lsm.range_scan(&"b", &"d");
        assert_eq!(range.len(), 3);
    }

    #[test]
    fn lsm_tree_overwrite() {
        let mut lsm = LSMTree::new(10);
        lsm.insert("x", 1);
        lsm.insert("x", 2);
        assert_eq!(lsm.get(&"x"), Some(2));
    }
}
