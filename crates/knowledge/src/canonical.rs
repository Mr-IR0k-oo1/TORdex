use serde::{Deserialize, Serialize};

/// A canonical representation of knowledge content.
///
/// Canonicalization normalizes equivalent representations into a single
/// standard form, enabling accurate deduplication and comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalForm {
    /// The normalized content as a stable JSON string.
    pub normalized: String,
    /// The schema or type of normalization applied.
    pub schema: String,
}

/// Normalization strategy for canonicalization.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Normalization {
    /// Sort all object keys recursively (canonical JSON).
    #[default]
    SortedKeys,
    /// Lowercase all string values.
    Lowercase,
    /// Strip all whitespace between tokens.
    StripWhitespace,
    /// Apply all normalizations above.
    Full,
}

/// A canonicalizer transforms knowledge content into a canonical form.
#[derive(Debug, Default)]
pub struct Canonicalizer {
    strategy: Normalization,
}

impl Canonicalizer {
    /// Create a new canonicalizer with the given strategy.
    #[must_use]
    pub fn new(strategy: Normalization) -> Self {
        Self { strategy }
    }

    /// Return the normalization strategy.
    #[must_use]
    pub fn strategy(&self) -> Normalization {
        self.strategy
    }

    /// Canonicalize a JSON value into its canonical form.
    #[must_use]
    pub fn canonicalize(&self, value: &serde_json::Value) -> CanonicalForm {
        let normalized = match self.strategy {
            Normalization::SortedKeys => sort_keys(value),
            Normalization::Lowercase => lowercase(value),
            Normalization::StripWhitespace => strip_whitespace(value),
            Normalization::Full => {
                let v = sort_keys(value);
                let v = lowercase(&v);
                strip_whitespace(&v)
            }
        };
        CanonicalForm {
            normalized: serde_json::to_string(&normalized).unwrap_or_default(),
            schema: format!("{:?}", self.strategy),
        }
    }
}

fn sort_keys(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted = serde_json::Map::new();
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for k in keys {
                sorted.insert(k.clone(), sort_keys(&map[k]));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(sort_keys).collect())
        }
        other => other.clone(),
    }
}

fn lowercase(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            serde_json::Value::String(s.to_lowercase())
        }
        serde_json::Value::Object(map) => {
            let mut next = serde_json::Map::new();
            for (k, v) in map {
                next.insert(k.to_lowercase(), lowercase(v));
            }
            serde_json::Value::Object(next)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(lowercase).collect())
        }
        other => other.clone(),
    }
}

fn strip_whitespace(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => {
            let stripped: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            serde_json::Value::String(stripped)
        }
        serde_json::Value::Object(map) => {
            let mut next = serde_json::Map::new();
            for (k, v) in map {
                next.insert(k.clone(), strip_whitespace(v));
            }
            serde_json::Value::Object(next)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(strip_whitespace).collect())
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorted_keys_produces_stable_json() {
        let c = Canonicalizer::new(Normalization::SortedKeys);
        let v1 = serde_json::json!({"z": 1, "a": 2, "m": 3});
        let v2 = serde_json::json!({"m": 3, "z": 1, "a": 2});
        assert_eq!(c.canonicalize(&v1), c.canonicalize(&v2));
    }

    #[test]
    fn lowercase_normalizes_strings() {
        let c = Canonicalizer::new(Normalization::Lowercase);
        let v = serde_json::json!({"name": "John Doe", "email": "User@Example.COM"});
        let form = c.canonicalize(&v);
        assert!(form.normalized.contains("john doe"));
        assert!(form.normalized.contains("user@example.com"));
    }

    #[test]
    fn strip_whitespace_removes_spaces() {
        let c = Canonicalizer::new(Normalization::StripWhitespace);
        let v = serde_json::json!({"code": "a b   c"});
        let form = c.canonicalize(&v);
        assert!(form.normalized.contains("abc"));
    }

    #[test]
    fn full_combines_all() {
        let c = Canonicalizer::new(Normalization::Full);
        let v = serde_json::json!({"B": "X Y", "A": "Hello World"});
        let form = c.canonicalize(&v);
        assert!(form.normalized.contains("\"a\""), "keys should be lowercased: {}", form.normalized);
        assert!(form.normalized.contains("\"b\""), "keys should be sorted: {}", form.normalized);
        assert!(!form.normalized.contains(' '), "whitespace should be stripped: {}", form.normalized);
        assert!(form.normalized.contains("helloworld"), "values should be lowercased: {}", form.normalized);
    }

    #[test]
    fn canonical_form_serializes() {
        let c = Canonicalizer::new(Normalization::SortedKeys);
        let v = serde_json::json!({"b": 2, "a": 1});
        let form = c.canonicalize(&v);
        let json = serde_json::to_string(&form).unwrap();
        let back: CanonicalForm = serde_json::from_str(&json).unwrap();
        assert_eq!(form, back);
    }
}
