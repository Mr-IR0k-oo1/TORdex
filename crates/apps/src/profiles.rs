//! Application profiles — each is a named composition of agents, watchers, and drivers.
//!
//! Every app shares the same kernel. Profiles differ only in which components
//! are activated, making each app a thin client over the same core.

/// An application profile describes which kernel components to enable.
#[derive(Debug, Clone)]
pub struct AppProfile {
    pub name: &'static str,
    pub description: &'static str,
    pub agents: &'static [&'static str],
    pub watchers: &'static [&'static str],
    pub drivers: &'static [&'static str],
    pub tags: &'static [&'static str],
}

// ─── Agent Key ─────────────────────────────────────────────────────────────
// research     — intelligence collection via drivers
// architecture — system component mapping
// malware      — binary artifact analysis
// monitoring   — kernel health monitoring
// documentation — system documentation
// forensics    — event timeline & relationship tracing
//
// monitoring-engine — registered separately when any watcher is active

// ─── Watcher Key ───────────────────────────────────────────────────────────
// repository   — git repository change detection
// onion        — Tor onion service status
// api          — API endpoint health & changes
// organization — org metadata changes
// package      — package registry changes
// cve          — CVE database updates
// threat       — threat intelligence feeds

// ─── Driver Key ────────────────────────────────────────────────────────────
// http         — fetch URLs, download content
// dns          — DNS resolution queries
// filesystem   — local file system access

// ─── Profiles ──────────────────────────────────────────────────────────────

/// TORdex Onion — Tor hidden service monitoring and intelligence.
pub const ONION: AppProfile = AppProfile {
    name: "TORdex Onion",
    description: "Tor onion service monitoring — tracks hidden service availability, content changes, and metadata.",
    agents: &["research", "monitoring"],
    watchers: &["onion"],
    drivers: &["http", "dns"],
    tags: &["tor", "onion", "hidden-services", "dark-web"],
};

/// TORdex OSINT — full-spectrum open-source intelligence platform.
pub const OSINT: AppProfile = AppProfile {
    name: "TORdex OSINT",
    description: "Full-spectrum open-source intelligence — all agents, watchers, and drivers enabled.",
    agents: &["research", "architecture", "malware", "monitoring", "documentation", "forensics"],
    watchers: &["repository", "onion", "api", "organization", "package", "cve", "threat"],
    drivers: &["http", "dns", "filesystem"],
    tags: &["osint", "intelligence", "full-spectrum"],
};

/// TORdex DFIR — digital forensics and incident response.
pub const DFIR: AppProfile = AppProfile {
    name: "TORdex DFIR",
    description: "Digital forensics and incident response — event timeline reconstruction, relationship tracing, and artifact analysis.",
    agents: &["forensics", "documentation", "research"],
    watchers: &["repository", "cve", "threat"],
    drivers: &["filesystem", "http"],
    tags: &["dfir", "forensics", "incident-response", "artifact-analysis"],
};

/// TORdex Malware — malware analysis and reverse engineering.
pub const MALWARE: AppProfile = AppProfile {
    name: "TORdex Malware",
    description: "Malware analysis — binary artifact inspection, threat detection, and behavioral tracking.",
    agents: &["malware", "forensics", "research"],
    watchers: &["cve", "threat"],
    drivers: &["http", "filesystem"],
    tags: &["malware", "reverse-engineering", "binary-analysis", "threat-detection"],
};

/// TORdex Research — intelligence research and collection.
pub const RESEARCH: AppProfile = AppProfile {
    name: "TORdex Research",
    description: "Intelligence research — data collection, entity discovery, relationship mapping, and system documentation.",
    agents: &["research", "architecture", "documentation"],
    watchers: &[],
    drivers: &["http", "dns", "filesystem"],
    tags: &["research", "intelligence", "collection", "discovery"],
};

/// TORdex Enterprise — full enterprise security suite.
pub const ENTERPRISE: AppProfile = AppProfile {
    name: "TORdex Enterprise",
    description: "Enterprise security suite — all capabilities, all agents, all watchers, all drivers.",
    agents: &["research", "architecture", "malware", "monitoring", "documentation", "forensics"],
    watchers: &["repository", "onion", "api", "organization", "package", "cve", "threat"],
    drivers: &["http", "dns", "filesystem"],
    tags: &["enterprise", "full-suite", "security"],
};

/// TORdex DevSecOps — development security pipeline integration.
pub const DEVSECOPS: AppProfile = AppProfile {
    name: "TORdex DevSecOps",
    description: "DevSecOps pipeline — repository monitoring, CVE tracking, architecture mapping, and system health.",
    agents: &["architecture", "monitoring", "documentation"],
    watchers: &["repository", "cve", "package"],
    drivers: &["http", "filesystem"],
    tags: &["devsecops", "pipeline", "security", "ci-cd"],
};

/// TORdex SOC — security operations center.
pub const SOC: AppProfile = AppProfile {
    name: "TORdex SOC",
    description: "Security operations — continuous monitoring, threat detection, CVE tracking, and incident investigation.",
    agents: &["monitoring", "forensics", "research", "malware"],
    watchers: &["repository", "onion", "api", "cve", "threat"],
    drivers: &["http", "dns"],
    tags: &["soc", "security-operations", "monitoring", "incident-response"],
};

/// TORdex API Observatory — API endpoint monitoring and intelligence.
pub const API_OBSERVATORY: AppProfile = AppProfile {
    name: "TORdex API Observatory",
    description: "API monitoring — endpoint health checks, change detection, version tracking, and usage analysis.",
    agents: &["research", "monitoring"],
    watchers: &["api", "repository"],
    drivers: &["http"],
    tags: &["api", "observatory", "endpoint", "monitoring"],
};

/// TORdex Repository Intelligence — repository analysis and tracking.
pub const REPOSITORY_INTELLIGENCE: AppProfile = AppProfile {
    name: "TORdex Repository Intelligence",
    description: "Repository intelligence — codebase analysis, dependency tracking, commit monitoring, and metadata collection.",
    agents: &["research", "architecture"],
    watchers: &["repository", "package"],
    drivers: &["http", "filesystem"],
    tags: &["repository", "intelligence", "code-analysis", "dependencies"],
};

/// All predefined profiles.
pub const ALL: &[AppProfile] = &[
    ONION, OSINT, DFIR, MALWARE, RESEARCH, ENTERPRISE, DEVSECOPS, SOC, API_OBSERVATORY,
    REPOSITORY_INTELLIGENCE,
];

/// Find a profile by exact name match.
#[must_use]
pub fn find(name: &str) -> Option<&'static AppProfile> {
    ALL.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// List all profile names.
#[must_use]
pub fn names() -> Vec<&'static str> {
    ALL.iter().map(|p| p.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_profiles_have_unique_names() {
        let mut seen = std::collections::HashSet::new();
        for p in ALL {
            assert!(seen.insert(p.name), "duplicate profile name: {}", p.name);
        }
    }

    #[test]
    fn all_profiles_have_descriptions() {
        for p in ALL {
            assert!(!p.description.is_empty(), "profile '{}' has no description", p.name);
        }
    }

    #[test]
    fn all_profiles_have_non_empty_agents() {
        for p in ALL {
            assert!(!p.agents.is_empty(), "profile '{}' has no agents", p.name);
        }
    }

    #[test]
    fn find_by_name_case_insensitive() {
        assert!(find("tordex osint").is_some());
        assert!(find("tordex dfir").is_some());
        assert!(find("nonexistent").is_none());
    }

    #[test]
    fn names_returns_all() {
        let n = names();
        assert_eq!(n.len(), 10);
        assert!(n.contains(&"TORdex OSINT"));
    }

    #[test]
    fn enterprise_has_all_agents() {
        assert_eq!(ENTERPRISE.agents.len(), 6);
    }

    #[test]
    fn research_has_no_watchers() {
        assert!(RESEARCH.watchers.is_empty());
    }

    #[test]
    fn onion_has_onion_watcher() {
        assert!(ONION.watchers.contains(&"onion"));
    }

    #[test]
    fn api_observatory_has_api_watcher() {
        assert!(API_OBSERVATORY.watchers.contains(&"api"));
    }

    #[test]
    fn repo_intel_has_repository_watcher() {
        assert!(REPOSITORY_INTELLIGENCE.watchers.contains(&"repository"));
    }

    #[test]
    fn enterprise_has_all_watchers() {
        assert_eq!(ENTERPRISE.watchers.len(), 7);
    }

    #[test]
    fn enterprise_has_all_drivers() {
        assert_eq!(ENTERPRISE.drivers.len(), 3);
    }
}
