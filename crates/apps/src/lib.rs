//! TORdex Application Profiles — thin client configurations sharing one kernel.
//!
//! Each app is a profile: which agents, watchers, drivers, and features to enable.
//! Apps are configuration, not separate binaries — every app runs on the same
//! kernel with the same crate set. Only the composition differs.

pub mod profiles;

use tordex_core::Kernel;

use crate::profiles::AppProfile;

/// Configure a kernel for the given application profile.
///
/// Registers the agents, watchers, and drivers specified by the profile.
/// Returns the kernel ready for use.
fn has(r#in: &[&str], item: &str) -> bool {
    r#in.iter().any(|&s| s == item)
}

pub fn configure(app: &AppProfile) -> Kernel {
    let kernel = Kernel::new();

    // Register agents selected by the profile
    for &name in app.agents {
        match name {
            "research" => {
                kernel.agents.register(Box::new(tordex_agents::ResearchAgent::new())).ok();
            }
            "architecture" => {
                kernel.agents.register(Box::new(tordex_agents::ArchitectureAgent::new())).ok();
            }
            "malware" => {
                kernel.agents.register(Box::new(tordex_agents::MalwareAgent::new())).ok();
            }
            "monitoring" => {
                kernel.agents.register(Box::new(tordex_agents::MonitoringAgent::new())).ok();
            }
            "documentation" => {
                kernel.agents.register(Box::new(tordex_agents::DocumentationAgent::new())).ok();
            }
            "forensics" => {
                kernel.agents.register(Box::new(tordex_agents::ForensicsAgent::new())).ok();
            }
            other => {
                tracing::warn!(agent = other, "unknown agent in profile");
            }
        }
    }

    // Register drivers if the profile requests them
    if has(app.drivers, "http") || has(app.drivers, "all") {
        kernel.drivers.register(Box::new(tordex_drivers::http::HttpDriver::new())).ok();
    }
    if has(app.drivers, "dns") || has(app.drivers, "all") {
        kernel.drivers.register(Box::new(tordex_drivers::dns::DnsDriver::new())).ok();
    }
    if has(app.drivers, "filesystem") || has(app.drivers, "all") {
        kernel.drivers.register(Box::new(tordex_drivers::filesystem::FilesystemDriver::new())).ok();
    }

    // Register watchers if the profile requests them
    if !app.watchers.is_empty() {
        let engine = std::sync::Arc::new(tordex_monitoring::MonitoringEngine::new());
        if has(app.watchers, "repository") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::RepositoryWatcher::new()));
        }
        if has(app.watchers, "onion") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::OnionWatcher::new()));
        }
        if has(app.watchers, "api") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::ApiWatcher::new()));
        }
        if has(app.watchers, "organization") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::OrganizationWatcher::new()));
        }
        if has(app.watchers, "package") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::PackageWatcher::new()));
        }
        if has(app.watchers, "cve") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::CveWatcher::new()));
        }
        if has(app.watchers, "threat") || has(app.watchers, "all") {
            engine.register(Box::new(tordex_monitoring::watchers::ThreatWatcher::new()));
        }
        let monitor_agent = tordex_monitoring::MonitoringAgent::new(engine);
        kernel.agents.register(Box::new(monitor_agent)).ok();
    }

    tracing::info!(
        app = %app.name,
        agents = %app.agents.join(","),
        drivers = %app.drivers.join(","),
        watchers = %app.watchers.join(","),
        "kernel configured"
    );

    kernel
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::*;

    #[test]
    fn onion_app_creates_kernel() {
        let app = ONION;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"research"));
        assert!(names.contains(&"monitoring"));
    }

    #[test]
    fn osint_app_has_all_core_agents() {
        let app = OSINT;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        for want in &["research", "architecture", "malware", "monitoring", "documentation", "forensics"] {
            assert!(names.contains(want), "OSINT app should register agent '{want}'");
        }
    }

    #[test]
    fn dfir_app_has_forensics() {
        let app = DFIR;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"forensics"));
        assert!(names.contains(&"documentation"));
    }

    #[test]
    fn malware_app_has_analysis_agents() {
        let app = MALWARE;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"malware"));
        assert!(names.contains(&"forensics"));
    }

    #[test]
    fn enterprise_app_has_all() {
        let app = ENTERPRISE;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        assert!(agents.len() >= 6);
    }

    #[test]
    fn soc_app_has_monitoring_watchers() {
        let app = SOC;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"monitoring-engine"));
    }

    #[test]
    fn research_app_has_research_and_docs() {
        let app = RESEARCH;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        let names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"research"));
        assert!(names.contains(&"documentation"));
    }

    #[test]
    fn repo_intel_app_has_related_agents() {
        let app = REPOSITORY_INTELLIGENCE;
        let kernel = configure(&app);
        let agents = kernel.agents.list();
        assert!(!agents.is_empty());
    }

    #[test]
    fn kernel_modules_all_initialized() {
        let app = OSINT;
        let kernel = configure(&app);
        assert_eq!(kernel.scheduler.running_count(), 0);
        assert!(kernel.objects.find_by_kind("entity").is_empty());
        assert_eq!(kernel.event_store.total_count(), 0);
    }
}
