use crate::{DependencyKind, Ecosystem, ManifestParser, ParsedDependency, ParsedManifest};

/// Parser for NuGet package manifests (.csproj, packages.config).
pub struct NugetParser;

impl ManifestParser for NugetParser {
    fn ecosystem(&self) -> Ecosystem {
        Ecosystem::NuGet
    }

    fn can_parse(&self, path: &str, _content: &str) -> bool {
        let lower = path.to_lowercase();
        lower.ends_with(".csproj") || lower.ends_with("packages.config")
    }

    fn parse(&self, path: &str, content: &str) -> Option<ParsedManifest> {
        let lower = path.to_lowercase();
        if lower.ends_with(".csproj") {
            parse_csproj(content)
        } else {
            parse_packages_config(content)
        }
    }
}

fn extract_xml_attr(line: &str, tag: &str, attr: &str) -> Option<String> {
    // Finds `<Tag attr="value"` or `<Tag attr='value'`
    let tag_open = format!("<{} ", tag);
    if let Some(tag_pos) = line.find(&tag_open) {
        let after_tag = &line[tag_pos + tag_open.len()..];
        let search = format!("{}=\"", attr);
        if let Some(attr_pos) = after_tag.find(&search) {
            let after_attr = &after_tag[attr_pos + search.len()..];
            if let Some(end) = after_attr.find('"') {
                return Some(after_attr[..end].to_string());
            }
        }
    }
    None
}

fn parse_csproj(content: &str) -> Option<ParsedManifest> {
    let mut dependencies = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(name) = extract_xml_attr(trimmed, "PackageReference", "Include") {
            let version = extract_xml_attr(trimmed, "PackageReference", "Version");
            dependencies.push(ParsedDependency {
                name,
                constraint: version,
                kind: DependencyKind::Runtime,
            });
        }
    }
    Some(ParsedManifest {
        ecosystem: Ecosystem::NuGet,
        package_name: {
            let name = content
                .lines()
                .find_map(|l| {
                    let t = l.trim();
                    extract_xml_attr(t, "PropertyGroup", "Include")
                        .or_else(|| {
                            let search = "<AssemblyName>";
                            t.find(search).and_then(|pos| {
                                let after = &t[pos + search.len()..];
                                after.find("</AssemblyName>").map(|end| after[..end].to_string())
                            })
                        })
                })
                .unwrap_or_else(|| "unknown".to_string());
            name
        },
        version: None,
        dependencies,
    })
}

fn parse_packages_config(content: &str) -> Option<ParsedManifest> {
    let mut dependencies = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(name) = extract_xml_attr(trimmed, "package", "id") {
            let version = extract_xml_attr(trimmed, "package", "version");
            dependencies.push(ParsedDependency {
                name,
                constraint: version,
                kind: DependencyKind::Runtime,
            });
        }
    }
    Some(ParsedManifest {
        ecosystem: Ecosystem::NuGet,
        package_name: "packages".to_string(),
        version: None,
        dependencies,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_csproj() {
        let content = r#"
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <AssemblyName>MyApp</AssemblyName>
  </PropertyGroup>
  <ItemGroup>
    <PackageReference Include="Newtonsoft.Json" Version="13.0.3" />
    <PackageReference Include="Serilog" Version="3.1.0" />
  </ItemGroup>
</Project>
"#;
        let manifest = NugetParser.parse("test.csproj", content).unwrap();
        assert_eq!(manifest.dependencies.len(), 2);
        assert_eq!(manifest.dependencies[0].name, "Newtonsoft.Json");
        assert_eq!(manifest.dependencies[0].constraint, Some("13.0.3".to_string()));
    }

    #[test]
    fn detect_nuget() {
        assert!(NugetParser.can_parse("test.csproj", ""));
        assert!(NugetParser.can_parse("packages.config", ""));
        assert!(!NugetParser.can_parse("Cargo.toml", ""));
    }
}
