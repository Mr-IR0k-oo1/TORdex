//! Domain watcher implementations.

mod repository;
mod onion;
mod api;
mod organization;
mod package;
mod cve;
mod threat;

pub use api::ApiWatcher;
pub use cve::CveWatcher;
pub use onion::OnionWatcher;
pub use organization::OrganizationWatcher;
pub use package::PackageWatcher;
pub use repository::RepositoryWatcher;
pub use threat::ThreatWatcher;

use tordex_core::Kernel;
use crate::watcher::Watcher;

/// Register all domain watchers into an engine.
pub fn register_all(engine: &crate::MonitoringEngine, _kernel: &Kernel) {
    let watchers: Vec<Box<dyn Watcher>> = vec![
        Box::new(RepositoryWatcher::new()),
        Box::new(OnionWatcher::new()),
        Box::new(ApiWatcher::new()),
        Box::new(OrganizationWatcher::new()),
        Box::new(PackageWatcher::new()),
        Box::new(CveWatcher::new()),
        Box::new(ThreatWatcher::new()),
    ];
    engine.register_all(watchers);
}
