#![forbid(unsafe_code)]
//! Collection Fabric — the orchestration layer for `TORdex` intelligence collection.
//!
//! Discovery → Priority Queue → Drivers → Sessions → Artifacts → Evidence
//!
//! ## Architecture
//!
//! ```text
//!        submit(task)
//!            │
//!            ▼
//!    PriorityQueue ──► CollectionFabric ──► DriverRegistry
//!            │              │                      │
//!            │              ▼                      ▼
//!            │       SessionManager          driver.execute()
//!            │              │                      │
//!            │              ▼                      ▼
//!            │       EventStore              CollectionSession
//!            │
//!     AdaptiveCrawler ◄── result
//! ```
//!
//! ## Modules
//!
//! | Module     | Description                                      |
//! |------------|--------------------------------------------------|
//! | `queue`    | Priority queue (`Priority`, `CollectionTask`)     |
//! | `session`  | Session state machine (`SessionState`, `SessionManager`) |
//! | `fabric`   | Collection orchestrator (`CollectionFabric`)      |
//! | `crawler`  | Adaptive web crawler (`AdaptiveCrawler`)          |

pub mod crawler;
pub mod fabric;
pub mod queue;
pub mod session;

pub use crawler::{AdaptiveCrawler, CrawlerConfig};
pub use fabric::CollectionFabric;
pub use queue::{CollectionTarget, CollectionTask, FabricError, Priority, PriorityQueue};
pub use session::{CollectionSession, SessionManager, SessionState};
