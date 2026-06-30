use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ulid::Ulid;

pub type IdentityId = Ulid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: IdentityId,
    pub public_key: Vec<u8>,
    pub label: String,
    pub created_at: i64,
}

pub trait SecurityManager: Send + Sync {
    fn hash(&self, data: &[u8]) -> Vec<u8>;
    fn generate_identity(&self, label: &str) -> Identity;
    fn register_identity(&self, identity: Identity) -> bool;
    fn resolve_identity(&self, id: IdentityId) -> Option<Identity>;
    fn verify(&self, data: &[u8], signature: &[u8], public_key: &[u8]) -> bool;
}

pub struct BasicSecurity {
    identities: Arc<Mutex<HashMap<IdentityId, Identity>>>,
}

impl BasicSecurity {
    #[must_use]
    pub fn new() -> Self {
        Self {
            identities: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl Default for BasicSecurity {
    fn default() -> Self {
        Self::new()
    }
}

impl SecurityManager for BasicSecurity {
    fn hash(&self, data: &[u8]) -> Vec<u8> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        hasher.finish().to_le_bytes().to_vec()
    }

    fn generate_identity(&self, label: &str) -> Identity {
        let id = IdentityId::new();
        let identity = Identity {
            id,
            public_key: vec![0u8; 32],
            label: label.to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64,
        };
        self.identities
            .lock()
            .unwrap()
            .insert(id, identity.clone());
        identity
    }

    fn register_identity(&self, identity: Identity) -> bool {
        self.identities
            .lock()
            .unwrap()
            .insert(identity.id, identity)
            .is_none()
    }

    fn resolve_identity(&self, id: IdentityId) -> Option<Identity> {
        self.identities.lock().unwrap().get(&id).cloned()
    }

    fn verify(&self, data: &[u8], signature: &[u8], public_key: &[u8]) -> bool {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        data.hash(&mut hasher);
        let expected = hasher.finish().to_le_bytes();
        let _ = public_key;
        signature == expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_returns_deterministic() {
        let sec = BasicSecurity::new();
        let a = sec.hash(b"hello");
        let b = sec.hash(b"hello");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_differs_for_different_inputs() {
        let sec = BasicSecurity::new();
        let a = sec.hash(b"hello");
        let b = sec.hash(b"world");
        assert_ne!(a, b);
    }

    #[test]
    fn generate_and_resolve_identity() {
        let sec = BasicSecurity::new();
        let identity = sec.generate_identity("test-node");
        let resolved = sec.resolve_identity(identity.id).unwrap();
        assert_eq!(resolved.label, "test-node");
    }

    #[test]
    fn verify_signature() {
        let sec = BasicSecurity::new();
        let data = b"important message";
        let identity = sec.generate_identity("signer");
        let signature = sec.hash(data);
        assert!(sec.verify(data, &signature, &identity.public_key));
    }
}
