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