use std::collections::HashMap;

/// A Trie (prefix tree) for efficient string prefix operations.
#[derive(Clone, Debug)]
pub struct Trie {
    children: HashMap<u8, Box<Trie>>,
    is_end: bool,
    value: Option<String>,
}

impl Trie {
    pub fn new() -> Self {
        Trie {
            children: HashMap::new(),
            is_end: false,
            value: None,
        }
    }

    pub fn insert(&mut self, key: &str, value: Option<String>) {
        let mut node = self;
        for &byte in key.as_bytes() {
            node = node
                .children
                .entry(byte)
                .or_insert_with(|| Box::new(Trie::new()));
        }
        node.is_end = true;
        if value.is_some() {
            node.value = value;
        }
    }

    pub fn search(&self, key: &str) -> bool {
        self.find_node(key).map_or(false, |n| n.is_end)
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        self.find_node(prefix).is_some()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.find_node(key)
            .and_then(|n| n.value.as_deref())
    }

    pub fn longest_prefix(&self, s: &str) -> Option<String> {
        let mut node = self;
        let mut last_match: Option<String> = None;
        for &byte in s.as_bytes() {
            match node.children.get(&byte) {
                Some(child) => {
                    if child.is_end {
                        last_match = Some(
                            s.as_bytes()[..=s.as_bytes().iter().position(|&b| b == byte).unwrap_or(0)]
                                .iter()
                                .map(|&b| b as char)
                                .collect(),
                        );
                    }
                    node = child;
                }
                None => break,
            }
        }
        last_match
    }

    pub fn keys_with_prefix(&self, prefix: &str) -> Vec<String> {
        let mut results = Vec::new();
        if let Some(node) = self.find_node(prefix) {
            node.collect_keys(&mut results, prefix.to_string());
        }
        results
    }

    fn find_node(&self, key: &str) -> Option<&Trie> {
        let mut node = self;
        for &byte in key.as_bytes() {
            node = node.children.get(&byte)?;
        }
        Some(node)
    }

    fn collect_keys(&self, results: &mut Vec<String>, prefix: String) {
        if self.is_end {
            results.push(prefix.clone());
        }
        for (&byte, child) in &self.children {
            let mut next = prefix.clone();
            next.push(byte as char);
            child.collect_keys(results, next);
        }
    }
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}

/// A Radix Tree (PATRICIA trie) with compressed paths.
#[derive(Clone, Debug)]
pub struct RadixTree {
    root: RadixNode,
}

#[derive(Clone, Debug)]
struct RadixNode {
    prefix: String,
    children: Vec<RadixNode>,
    is_end: bool,
    value: Option<String>,
}

impl RadixTree {
    pub fn new() -> Self {
        RadixTree {
            root: RadixNode {
                prefix: String::new(),
                children: Vec::new(),
                is_end: false,
                value: None,
            },
        }
    }

    pub fn insert(&mut self, key: &str, value: Option<String>) {
        self.root.insert(key, value);
    }

    pub fn search(&self, key: &str) -> bool {
        self.root.search(key)
    }

    pub fn starts_with(&self, prefix: &str) -> bool {
        self.root.starts_with(prefix)
    }

    pub fn longest_prefix(&self, s: &str) -> Option<String> {
        self.root.longest_prefix(s)
    }
}

impl Default for RadixTree {
    fn default() -> Self {
        Self::new()
    }
}

impl RadixNode {
    fn insert(&mut self, key: &str, value: Option<String>) {
        if key.is_empty() {
            self.is_end = true;
            if value.is_some() {
                self.value = value;
            }
            return;
        }
        let common_prefix = self.common_prefix_len(key);
        if common_prefix > 0 && common_prefix < self.prefix.len() {
            let existing_suffix = self.prefix[common_prefix..].to_string();
            let child = RadixNode {
                prefix: existing_suffix,
                children: std::mem::take(&mut self.children),
                is_end: self.is_end,
                value: self.value.take(),
            };
            self.prefix = self.prefix[..common_prefix].to_string();
            self.children = vec![child];
            self.is_end = false;

            let rest = &key[common_prefix..];
            let new_node = RadixNode {
                prefix: rest.to_string(),
                children: Vec::new(),
                is_end: true,
                value,
            };
            self.children.push(new_node);
            return;
        }
        if common_prefix > 0 && common_prefix == self.prefix.len() {
            let rest = &key[common_prefix..];
            if rest.is_empty() {
                self.is_end = true;
                if value.is_some() {
                    self.value = value;
                }
                return;
            }
            if let Some(child) = self
                .children
                .iter_mut()
                .find(|c| !c.prefix.is_empty() && c.prefix.as_bytes()[0] == rest.as_bytes()[0])
            {
                child.insert(rest, value);
                return;
            }
            let new_node = RadixNode {
                prefix: rest.to_string(),
                children: Vec::new(),
                is_end: true,
                value,
            };
            self.children.push(new_node);
            return;
        }
        if common_prefix == 0 {
            if let Some(child) = self
                .children
                .iter_mut()
                .find(|c| !c.prefix.is_empty() && !key.is_empty() && c.prefix.as_bytes()[0] == key.as_bytes()[0])
            {
                child.insert(key, value);
                return;
            }
            let new_node = RadixNode {
                prefix: key.to_string(),
                children: Vec::new(),
                is_end: true,
                value,
            };
            self.children.push(new_node);
        }
    }

    fn search(&self, key: &str) -> bool {
        if key.is_empty() {
            return self.is_end;
        }
        if key.starts_with(&self.prefix) {
            let rest = &key[self.prefix.len()..];
            if rest.is_empty() {
                return self.is_end;
            }
            for child in &self.children {
                if !child.prefix.is_empty() && !rest.is_empty()
                    && child.prefix.as_bytes()[0] == rest.as_bytes()[0]
                {
                    return child.search(rest);
                }
            }
        }
        false
    }

    fn starts_with(&self, prefix: &str) -> bool {
        if prefix.is_empty() {
            return true;
        }
        if prefix.starts_with(&self.prefix) {
            let rest = &prefix[self.prefix.len()..];
            if rest.is_empty() {
                return true;
            }
            for child in &self.children {
                if !child.prefix.is_empty() && !rest.is_empty()
                    && child.prefix.as_bytes()[0] == rest.as_bytes()[0]
                {
                    return child.starts_with(rest);
                }
            }
            return false;
        }
        if self.prefix.starts_with(prefix) {
            return true;
        }
        false
    }

    fn longest_prefix(&self, s: &str) -> Option<String> {
        if s.is_empty() {
            return if self.is_end {
                Some(self.prefix.clone())
            } else {
                None
            };
        }
        if s.starts_with(&self.prefix) {
            let rest = &s[self.prefix.len()..];
            for child in &self.children {
                if let Some(result) = child.longest_prefix(rest) {
                    let mut full = self.prefix.clone();
                    full.push_str(&result);
                    return Some(full);
                }
            }
            if self.is_end {
                return Some(self.prefix.clone());
            }
        }
        None
    }

    fn common_prefix_len(&self, s: &str) -> usize {
        self.prefix
            .as_bytes()
            .iter()
            .zip(s.as_bytes())
            .take_while(|(a, b)| a == b)
            .count()
    }
}

/// FM Index for full-text substring search using Burrows-Wheeler Transform.
#[derive(Clone, Debug)]
pub struct FMIndex {
    #[allow(dead_code)]
    bwt: Vec<u8>,
    suffix_array: Vec<usize>,
    first_occurrence: HashMap<u8, usize>,
    count: Vec<[usize; 256]>,
    text_len: usize,
}

impl FMIndex {
    pub fn new(text: &str) -> Self {
        let text_bytes = text.as_bytes();
        let n = text_bytes.len() + 1;
        let mut extended = Vec::with_capacity(n);
        extended.extend_from_slice(text_bytes);
        extended.push(0u8);

        let mut suffixes: Vec<(usize, Vec<u8>)> = (0..n)
            .map(|i| {
                let suffix = extended[i..].to_vec();
                (i, suffix)
            })
            .collect();
        suffixes.sort_by(|a, b| a.1.cmp(&b.1));

        let suffix_array: Vec<usize> = suffixes.iter().map(|(i, _)| *i).collect();
        let bwt: Vec<u8> = suffixes
            .iter()
            .map(|(i, _)| {
                if *i == 0 {
                    b'\0'
                } else {
                    extended[i - 1]
                }
            })
            .collect();

        let mut first_occurrence = HashMap::new();
        for (i, &(_, ref s)) in suffixes.iter().enumerate() {
            first_occurrence.entry(s[0]).or_insert(i);
        }

        let mut count = vec![[0usize; 256]; n + 1];
        for (i, &c) in bwt.iter().enumerate() {
            count[i + 1] = count[i];
            count[i + 1][c as usize] += 1;
        }

        FMIndex {
            bwt,
            suffix_array,
            first_occurrence,
            count,
            text_len: n,
        }
    }

    pub fn count_occurrences(&self, pattern: &str) -> usize {
        if pattern.is_empty() {
            return 0;
        }
        let pattern_bytes = pattern.as_bytes();
        let mut l = 0usize;
        let mut r = self.text_len;
        let mut i = pattern_bytes.len();
        loop {
            if i == 0 {
                return r.saturating_sub(l);
            }
            i -= 1;
            let c = pattern_bytes[i];
            l = self.first_occurrence.get(&c).copied().unwrap_or(0)
                + self.count[l][c as usize];
            r = self.first_occurrence.get(&c).copied().unwrap_or(0)
                + self.count[r][c as usize];
            if l >= r {
                return 0;
            }
        }
    }

    pub fn search(&self, pattern: &str) -> Vec<usize> {
        let count = self.count_occurrences(pattern);
        if count == 0 {
            return Vec::new();
        }
        let pattern_bytes = pattern.as_bytes();
        let mut l = 0usize;
        let mut r = self.text_len;
        let mut i = pattern_bytes.len();
        loop {
            if i == 0 {
                break;
            }
            i -= 1;
            let c = pattern_bytes[i];
            l = self.first_occurrence.get(&c).copied().unwrap_or(0)
                + self.count[l][c as usize];
            r = self.first_occurrence.get(&c).copied().unwrap_or(0)
                + self.count[r][c as usize];
        }
        self.suffix_array[l..r].to_vec()
    }
}

/// An Inverted Index mapping terms to document positions.
#[derive(Clone, Debug)]
pub struct InvertedIndex {
    index: HashMap<String, Vec<(String, Vec<usize>)>>,
    doc_count: usize,
}

impl InvertedIndex {
    pub fn new() -> Self {
        InvertedIndex {
            index: HashMap::new(),
            doc_count: 0,
        }
    }

    pub fn add_document(&mut self, doc_id: &str, text: &str) {
        self.doc_count += 1;
        let terms: Vec<String> = text
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();

        let mut term_positions: HashMap<String, Vec<usize>> = HashMap::new();
        for (pos, term) in terms.iter().enumerate() {
            term_positions
                .entry(term.clone())
                .or_default()
                .push(pos);
        }

        for (term, positions) in term_positions {
            self.index
                .entry(term)
                .or_default()
                .push((doc_id.to_string(), positions));
        }
    }

    pub fn search(&self, query: &str) -> Vec<(String, Vec<usize>)> {
        let term = query.to_lowercase();
        self.index
            .get(&term)
            .cloned()
            .unwrap_or_default()
    }

    pub fn search_multi(&self, query: &str) -> HashMap<String, Vec<(String, Vec<usize>)>> {
        let mut results = HashMap::new();
        for term in query
            .to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
        {
            if let Some(postings) = self.index.get(term) {
                results.insert(term.to_string(), postings.clone());
            }
        }
        results
    }

    pub fn term_frequency(&self, term: &str, doc_id: &str) -> usize {
        let term = term.to_lowercase();
        self.index
            .get(&term)
            .map(|postings| {
                postings
                    .iter()
                    .filter(|(id, _)| id == doc_id)
                    .map(|(_, positions)| positions.len())
                    .sum()
            })
            .unwrap_or(0)
    }

    pub fn document_frequency(&self, term: &str) -> usize {
        let term = term.to_lowercase();
        self.index
            .get(&term)
            .map(|postings| postings.len())
            .unwrap_or(0)
    }

    pub fn inverse_document_frequency(&self, term: &str) -> f64 {
        let df = self.document_frequency(term);
        if df == 0 {
            return 0.0;
        }
        (self.doc_count as f64 / df as f64).ln()
    }

    pub fn terms(&self) -> Vec<String> {
        let mut terms: Vec<String> = self.index.keys().cloned().collect();
        terms.sort();
        terms
    }

    pub fn doc_count(&self) -> usize {
        self.doc_count
    }
}

impl Default for InvertedIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trie_basic_insert_search() {
        let mut t = Trie::new();
        t.insert("hello", None);
        t.insert("world", None);
        assert!(t.search("hello"));
        assert!(t.search("world"));
        assert!(!t.search("hell"));
        assert!(!t.search("worlds"));
    }

    #[test]
    fn trie_starts_with() {
        let mut t = Trie::new();
        t.insert("hello", None);
        t.insert("help", None);
        assert!(t.starts_with("hel"));
        assert!(t.starts_with("hello"));
        assert!(!t.starts_with("hex"));
    }

    #[test]
    fn trie_keys_with_prefix() {
        let mut t = Trie::new();
        t.insert("apple", None);
        t.insert("appetite", None);
        t.insert("app", None);
        let keys = t.keys_with_prefix("app");
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"app".to_string()));
    }

    #[test]
    fn trie_get_value() {
        let mut t = Trie::new();
        t.insert("key", Some("value".to_string()));
        assert_eq!(t.get("key"), Some("value"));
    }

    #[test]
    fn radix_tree_basic() {
        let mut rt = RadixTree::new();
        rt.insert("hello", None);
        rt.insert("world", None);
        assert!(rt.search("hello"));
        assert!(rt.search("world"));
        assert!(!rt.search("hell"));
    }

    #[test]
    fn radix_tree_starts_with() {
        let mut rt = RadixTree::new();
        rt.insert("test", None);
        rt.insert("testing", None);
        assert!(rt.starts_with("test"));
        assert!(rt.starts_with("te"));
        assert!(!rt.starts_with("xyz"));
    }

    #[test]
    fn fm_index_basic_search() {
        let fm = FMIndex::new("banana");
        assert_eq!(fm.count_occurrences("ana"), 2);
        assert_eq!(fm.count_occurrences("ban"), 1);
        assert_eq!(fm.count_occurrences("x"), 0);
    }

    #[test]
    fn fm_index_positions() {
        let fm = FMIndex::new("abcabc");
        let positions = fm.search("abc");
        assert_eq!(positions.len(), 2);
    }

    #[test]
    fn inverted_index_basic() {
        let mut idx = InvertedIndex::new();
        idx.add_document("doc1", "hello world");
        idx.add_document("doc2", "hello foo");
        let results = idx.search("hello");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn inverted_index_multi_term() {
        let mut idx = InvertedIndex::new();
        idx.add_document("doc1", "the quick brown fox");
        idx.add_document("doc2", "the lazy dog");
        let multi = idx.search_multi("the fox");
        assert_eq!(multi.len(), 2);
    }

    #[test]
    fn inverted_index_tf_idf() {
        let mut idx = InvertedIndex::new();
        idx.add_document("d1", "apple banana");
        idx.add_document("d2", "apple cherry");
        assert_eq!(idx.term_frequency("apple", "d1"), 1);
        assert_eq!(idx.document_frequency("apple"), 2);
        let idf = idx.inverse_document_frequency("apple");
        assert!(idf >= 0.0);
    }
}
