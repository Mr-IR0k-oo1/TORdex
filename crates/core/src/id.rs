//! Typed identifiers.
//!
//! Each ID is a thin newtype over a ULID. ULIDs are 128-bit, lexicographically
//! sortable, and embed a millisecond-precision timestamp, which makes them
//! ideal for the temporal queries that dominate later layers of TORdex.

use serde::{Deserialize, Serialize};
use std::fmt;
use ulid::Ulid;

macro_rules! id_newtype {
    ($name:ident) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Ulid);

        impl $name {
            /// Generate a fresh identifier using the current time.
            #[must_use]
            pub fn generate() -> Self {
                Self(Ulid::new())
            }

            /// Construct from the raw string representation. Returns `None` if invalid.
            #[must_use]
            pub fn from_str(s: &str) -> Option<Self> {
                Ulid::from_string(s).ok().map(Self)
            }

            /// Borrow the inner ULID.
            #[must_use]
            pub fn inner(self) -> Ulid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl std::str::FromStr for $name {
            type Err = ulid::DecodeError;

            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                Ulid::from_string(s).map(Self)
            }
        }
    };
}

id_newtype!(SourceId);
id_newtype!(CollectionId);
id_newtype!(SessionId);
id_newtype!(EvidenceId);
id_newtype!(ServiceId);
id_newtype!(ArtifactId);
id_newtype!(EventId);
id_newtype!(KnowledgeId);
id_newtype!(RelationshipId);
id_newtype!(ObservationId);
id_newtype!(EntityId);
id_newtype!(FindingId);
id_newtype!(DecisionId);
id_newtype!(TimelineId);
id_newtype!(InvestigationId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_creates_valid_ulid() {
        let id = EventId::generate();
        let s = id.to_string();
        assert_eq!(s.len(), 26);
        assert!(Ulid::from_string(&s).is_ok());
    }

    #[test]
    fn from_str_parses_valid() {
        let raw = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let id = EventId::from_str(raw);
        assert!(id.is_some());
        assert_eq!(id.unwrap().to_string(), raw);
    }

    #[test]
    fn from_str_rejects_invalid() {
        assert!(EventId::from_str("not-a-ulid").is_none());
        assert!(EventId::from_str("").is_none());
    }

    #[test]
    fn display_roundtrip() {
        let a = EventId::generate();
        let s = a.to_string();
        let b = EventId::from_str(&s).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn from_str_trait() {
        let raw = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
        let id: EventId = raw.parse().unwrap();
        assert_eq!(id.to_string(), raw);
    }

    #[test]
    fn ids_are_distinct() {
        let a = EventId::generate();
        let b = EventId::generate();
        assert_ne!(a, b);
    }

    #[test]
    fn serde_roundtrip() {
        let id = ServiceId::generate();
        let json = serde_json::to_string(&id).unwrap();
        let deserialized: ServiceId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn inner_returns_ulid() {
        let id = ServiceId::generate();
        let ulid = id.inner();
        assert_eq!(id.to_string(), ulid.to_string());
    }

    #[test]
    fn ord_is_consistent() {
        let a = SessionId::generate();
        let b = a;
        assert_eq!(a, b);
        assert!(!(a < b));
    }
}