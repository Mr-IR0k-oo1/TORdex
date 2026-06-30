use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for Gradle build files (build.gradle, build.gradle.kts).
pub struct GradleParser;

impl ManifestParser for GradleParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::Gradle
    }

    fn can_parse(&self, path: &str, _content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with("build.gradle") || lower.ends_with("build.gradle.kts")
    }

    fn parse(&self, _path: &str, content: &str) -> Option<ParsedManifest> {
        let lines: Vec<&str> = content.lines().collect();
        let mut package_name = None;
        let version = None;
        let mut dependencies = Vec::new();
        let mut in_deps_block = false;
        let mut brace_depth = 0;

        // Try to find root project name
        for line in &lines {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix("rootProject.name = ")
                .or_else(|| trimmed.strip_prefix("rootProject.name="))
            {
                package_name = Some(name.trim().trim_matches('"').trim_matches('\'').to_string());
                break;
            }
        }

        for line in &lines {
            let trimmed = line.trim();

            if trimmed.starts_with("dependencies") || trimmed.starts_with("dependencies ") {
                in_deps_block = true;
                brace_depth = 0;
                // Handle `dependencies {` on same or next line
                if trimmed.contains('{') {
                    brace_depth = 1;
                }
                continue;
            }

            if in_deps_block {
                for ch in trimmed.chars() {
                    if ch == '{' {
                        brace_depth += 1;
                    } else if ch == '}' {
                        brace_depth -= 1;
                    }
                }
                if brace_depth <= 0 {
                    in_deps_block = false;
                    continue;
                }

                let dep_str = trimmed;
                // Skip configuration lines and comments
                if dep_str.starts_with("//") || dep_str.starts_with("/*") {
                    continue;
                }

                // Match: configuration "group:name:version" or configuration("group:name:version")
                if let Some(rest) = dep_str
                    .strip_prefix("implementation ")
                    .or_else(|| dep_str.strip_prefix("implementation("))
                    .or_else(|| dep_str.strip_prefix("api "))
                    .or_else(|| dep_str.strip_prefix("api("))
                    .or_else(|| dep_str.strip_prefix("compileOnly "))
                    .or_else(|| dep_str.strip_prefix("compileOnly("))
                    .or_else(|| dep_str.strip_prefix("runtimeOnly "))
                    .or_else(|| dep_str.strip_prefix("runtimeOnly("))
                    .or_else(|| dep_str.strip_prefix("testImplementation "))
                    .or_else(|| dep_str.strip_prefix("testImplementation("))
                {
                    let dep_spec = rest
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .trim_end_matches(')');
                    let parts: Vec<&str> = dep_spec.split(':').collect();
                    if parts.len() >= 2 {
                        let name = format!("{}:{}", parts[0], parts[1]);
                        let constraint = parts.get(2).map(|s| s.to_string());
                        let kind = if dep_str.starts_with("test") {
                            DependencyKind::Dev
                        } else {
                            DependencyKind::Runtime
                        };
                        dependencies.push(ParsedDependency {
                            name,
                            constraint,
                            kind,
                        });
                    }
                }
            }
        }

        let package_name = package_name.unwrap_or_else(|| "project".to_string());
        Some(ParsedManifest {
            ecosystem: Ecosystem::Gradle,
            package_name,
            version,
            dependencies,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_build_gradle() {
        let content = r#"
rootProject.name = "my-app"

dependencies {
    implementation 'org.springframework:spring-core:6.0.0'
    testImplementation 'junit:junit:5.0.0'
    api 'com.google.guava:guava:33.0.0'
}
"#;
        let manifest = GradleParser.parse("build.gradle", content).unwrap();
        assert_eq!(manifest.package_name, "my-app");
        assert_eq!(manifest.dependencies.len(), 3);
        assert_eq!(manifest.dependencies[0].name, "org.springframework:spring-core");
        assert_eq!(manifest.dependencies[1].kind, DependencyKind::Dev);
    }

    #[test]
    fn detect_gradle() {
        assert!(GradleParser.can_parse("build.gradle", ""));
        assert!(GradleParser.can_parse("build.gradle.kts", ""));
        assert!(!GradleParser.can_parse("Cargo.toml", ""));
    }
}
