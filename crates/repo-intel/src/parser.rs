use serde::{Deserialize, Serialize};

use crate::Language;

/// A source code location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLocation {
    pub file: String,
    pub line: usize,
    pub column: usize,
}

/// Visibility of a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

/// Kinds of code symbols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolKind {
    Function,
    Method,
    Class,
    Struct,
    Interface,
    Trait,
    Enum,
    Module,
    Namespace,
    Variable,
    Constant,
    Macro,
    Type,
}

/// A named symbol extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub location: CodeLocation,
    pub visibility: Visibility,
    pub signature: String,
}

/// An import statement extracted from source code.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Import {
    pub source: String,
    pub names: Vec<String>,
    pub location: CodeLocation,
}

/// A parsed source module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub path: String,
    pub language: Language,
    pub symbols: Vec<CodeSymbol>,
    pub imports: Vec<Import>,
    pub line_count: usize,
}

/// A package (group of related modules).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub language: Language,
    pub modules: Vec<Module>,
    pub total_lines: usize,
}

/// A workspace (group of packages).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub root: String,
    pub packages: Vec<Package>,
}

/// A source code analyzer that extracts symbols and structure
/// from source files across all supported languages.
#[derive(Debug)]
pub struct SourceAnalyzer;

impl SourceAnalyzer {
    /// Analyze a single source file, extracting symbols and imports.
    #[must_use]
    pub fn analyze_file(path: &str, content: &str) -> Option<Module> {
        let language = Language::from_filename(path)
            .or_else(|| Language::from_content(content))?;
        let line_count = content.lines().count();

        let symbols = Self::extract_symbols(content, &language, path);
        let imports = Self::extract_imports(content, &language, path);

        Some(Module {
            path: path.to_string(),
            language,
            symbols,
            imports,
            line_count,
        })
    }

    /// Extract symbols from source code for a given language.
    fn extract_symbols(content: &str, language: &Language, file: &str) -> Vec<CodeSymbol> {
        match language {
            Language::Rust => Self::parse_rust_symbols(content, file),
            Language::Python => Self::parse_python_symbols(content, file),
            Language::Go => Self::parse_go_symbols(content, file),
            Language::Java => Self::parse_java_symbols(content, file),
            Language::JavaScript | Language::TypeScript => {
                Self::parse_js_symbols(content, file)
            }
            Language::C | Language::Cpp => Self::parse_c_cpp_symbols(content, file),
            Language::CSharp => Self::parse_csharp_symbols(content, file),
            Language::PHP => Self::parse_php_symbols(content, file),
            Language::Kotlin => Self::parse_kotlin_symbols(content, file),
            Language::Swift => Self::parse_swift_symbols(content, file),
            Language::Zig => Self::parse_zig_symbols(content, file),
            Language::Lua => Self::parse_lua_symbols(content, file),
            Language::Wasm => vec![],
        }
    }

    /// Extract import/use/include statements.
    fn extract_imports(content: &str, language: &Language, file: &str) -> Vec<Import> {
        match language {
            Language::Rust => Self::parse_rust_imports(content, file),
            Language::Python => Self::parse_python_imports(content, file),
            Language::Go => Self::parse_go_imports(content, file),
            Language::Java => Self::parse_java_imports(content, file),
            Language::JavaScript | Language::TypeScript => {
                Self::parse_js_imports(content, file)
            }
            Language::C | Language::Cpp => Self::parse_c_imports(content, file),
            Language::CSharp => Self::parse_csharp_imports(content, file),
            Language::PHP => Self::parse_php_imports(content, file),
            Language::Kotlin => Self::parse_kotlin_imports(content, file),
            Language::Swift => Self::parse_swift_imports(content, file),
            Language::Zig => Self::parse_zig_imports(content, file),
            Language::Lua => Self::parse_lua_imports(content, file),
            Language::Wasm => vec![],
        }
    }

    fn loc(line: usize, file: &str) -> CodeLocation {
        CodeLocation {
            file: file.to_string(),
            line,
            column: 0,
        }
    }

    // ── Rust ────────────────────────────────────────────────────────────

    fn parse_rust_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("pub fn ").and_then(|s| {
                s.split('(').next().map(|s| s.trim())
            }) {
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("fn ").and_then(|s| {
                s.split('(').next().map(|s| s.trim())
            }) {
                if !name.starts_with("fn ") {
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Function,
                        location: Self::loc(ln, file),
                        visibility: Visibility::Private,
                        signature: line.to_string(),
                    });
                }
            } else if let Some(name) = line.strip_prefix("pub struct ") {
                symbols.push(CodeSymbol {
                    name: name.split(&['<', '{', ' ', '\t'][..]).next().unwrap_or(name).trim().to_string(),
                    kind: SymbolKind::Struct,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("pub enum ") {
                symbols.push(CodeSymbol {
                    name: name.split(&['<', '{', ' ', '\t'][..]).next().unwrap_or(name).trim().to_string(),
                    kind: SymbolKind::Enum,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("pub trait ") {
                symbols.push(CodeSymbol {
                    name: name.split(&['<', '{', ' ', '\t'][..]).next().unwrap_or(name).trim().to_string(),
                    kind: SymbolKind::Trait,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("pub type ") {
                symbols.push(CodeSymbol {
                    name: name.split('=').next().unwrap_or(name).trim().to_string(),
                    kind: SymbolKind::Type,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_rust_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(use_path) = line.strip_prefix("use ") {
                let path = use_path.trim_end_matches(';').trim();
                if !path.starts_with("crate::") && !path.starts_with("self::")
                    && !path.starts_with("super::")
                {
                    let names = vec![path.rsplit("::").next().unwrap_or("").to_string()];
                    imports.push(Import {
                        source: path.to_string(),
                        names,
                        location: Self::loc(ln, file),
                    });
                }
            }
        }
        imports
    }

    // ── Python ──────────────────────────────────────────────────────────

    fn parse_python_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("def ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("class ") {
                let name = name.split(':').next().unwrap_or(name).split('(').next().unwrap_or(name).trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_python_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(module) = line.strip_prefix("import ") {
                for name in module.split(',') {
                    let name = name.trim();
                    imports.push(Import {
                        source: name.to_string(),
                        names: vec![],
                        location: Self::loc(ln, file),
                    });
                }
            } else if let Some(from_part) = line.strip_prefix("from ") {
                let parts: Vec<&str> = from_part.splitn(2, " import ").collect();
                if parts.len() == 2 {
                    let source = parts[0].trim();
                    let names: Vec<String> = parts[1].split(',').map(|s| s.trim().to_string()).collect();
                    imports.push(Import {
                        source: source.to_string(),
                        names,
                        location: Self::loc(ln, file),
                    });
                }
            }
        }
        imports
    }

    // ── Go ──────────────────────────────────────────────────────────────

    fn parse_go_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("func ") {
                let name = name.split('(').next().unwrap_or("").trim();
                let is_method = name.contains(')');
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: if is_method { SymbolKind::Method } else { SymbolKind::Function },
                    location: Self::loc(ln, file),
                    visibility: if name.as_bytes().first().map(|b| b.is_ascii_uppercase()).unwrap_or(false) {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    },
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("type ") {
                if name.contains("struct") || name.contains("interface") {
                    let name = name.split(' ').next().unwrap_or("").trim();
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Struct,
                        location: Self::loc(ln, file),
                        visibility: Visibility::Public,
                        signature: line.to_string(),
                    });
                }
            }
        }
        symbols
    }

    fn parse_go_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        let mut in_import_block = false;
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if line == "import (" {
                in_import_block = true;
                continue;
            }
            if in_import_block {
                if line == ")" {
                    in_import_block = false;
                    continue;
                }
                let path = line.trim_matches('"');
                if !path.is_empty() {
                    imports.push(Import {
                        source: path.to_string(),
                        names: vec![],
                        location: Self::loc(ln, file),
                    });
                }
            }
            if let Some(path) = line.strip_prefix("import ") {
                let path = path.trim_matches('"');
                imports.push(Import {
                    source: path.to_string(),
                    names: vec![],
                    location: Self::loc(ln, file),
                });
            }
        }
        imports
    }

    // ── Java ────────────────────────────────────────────────────────────

    fn parse_java_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(rest) = line.strip_prefix("public class ")
                .or_else(|| line.strip_prefix("class "))
            {
                let name = rest.split('<').next().unwrap_or(rest).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix("public interface ") {
                let name = rest.split('<').next().unwrap_or(rest).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix("public enum ") {
                let name = rest.split('<').next().unwrap_or(rest).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Enum,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
            // Method detection
            if line.contains('(') && line.contains(')') && !line.contains(';')
                && (line.starts_with("public ") || line.starts_with("private ")
                    || line.starts_with("protected "))
            {
                if let Some(name) = line.split('(').next() {
                    let name = name.split(' ').last().unwrap_or("");
                    if !name.is_empty() && name != "class" && name != "interface" {
                        symbols.push(CodeSymbol {
                            name: name.to_string(),
                            kind: SymbolKind::Method,
                            location: Self::loc(ln, file),
                            visibility: if line.starts_with("public") { Visibility::Public }
                                else if line.starts_with("private") { Visibility::Private }
                                else { Visibility::Protected },
                            signature: line.to_string(),
                        });
                    }
                }
            }
        }
        symbols
    }

    fn parse_java_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(path) = line.strip_prefix("import ") {
                let path = path.trim_end_matches(';').trim();
                if !path.starts_with("java.") && !path.starts_with("javax.") {
                    let names = vec![path.rsplit('.').next().unwrap_or("").to_string()];
                    imports.push(Import {
                        source: path.to_string(),
                        names,
                        location: Self::loc(ln, file),
                    });
                }
            }
        }
        imports
    }

    // ── JavaScript / TypeScript ─────────────────────────────────────────

    fn parse_js_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("function ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("export function ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("class ") {
                let name = name.split('{').next().unwrap_or(name).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("interface ") {
                let name = name.split('{').next().unwrap_or(name).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(_rest) = line.strip_prefix("const ") {
                if let Some(name) = line.split('=').next() {
                    let name = name.strip_prefix("const ").unwrap_or(name).trim();
                    if !name.starts_with('{') {
                        symbols.push(CodeSymbol {
                            name: name.to_string(),
                            kind: SymbolKind::Variable,
                            location: Self::loc(ln, file),
                            visibility: Visibility::Private,
                            signature: line.to_string(),
                        });
                    }
                }
            }
        }
        symbols
    }

    fn parse_js_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(rest) = line.strip_prefix("import ") {
                let trimmed = rest.trim_end_matches(';');
                // import { a, b } from "module"
                if let Some(from_pos) = trimmed.find(" from ") {
                    let source = trimmed[from_pos + 6..].trim().trim_matches('"').trim_matches('\'');
                    imports.push(Import {
                        source: source.to_string(),
                        names: vec![],
                        location: Self::loc(ln, file),
                    });
                } else {
                    // import "module"
                    let source = trimmed.trim().trim_matches('"').trim_matches('\'');
                    if !source.is_empty() {
                        imports.push(Import {
                            source: source.to_string(),
                            names: vec![],
                            location: Self::loc(ln, file),
                        });
                    }
                }
            }
            // require
            if let Some(pos) = line.find("require(") {
                let rest = &line[pos + 8..];
                if let Some(end) = rest.find(')') {
                    let source = rest[..end].trim().trim_matches('"').trim_matches('\'');
                    imports.push(Import {
                        source: source.to_string(),
                        names: vec![],
                        location: Self::loc(ln, file),
                    });
                }
            }
        }
        imports
    }

    // ── C / C++ ─────────────────────────────────────────────────────────

    fn parse_c_cpp_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            // Function definitions (return_type name(...) {)
            if line.ends_with('{') && !line.starts_with("if ")
                && !line.starts_with("while ") && !line.starts_with("for ")
                && !line.starts_with("switch ") && !line.starts_with("struct ")
            {
                if let Some(name) = line.split('(').next() {
                    let name = name.split(' ').last().unwrap_or("").trim();
                    if !name.is_empty() && !name.starts_with('*') && !name.contains("if")
                        && !name.contains("return")
                    {
                        symbols.push(CodeSymbol {
                            name: name.to_string(),
                            kind: SymbolKind::Function,
                            location: Self::loc(ln, file),
                            visibility: Visibility::Public,
                            signature: line.to_string(),
                        });
                    }
                }
            }
            if line.starts_with("struct ") && line.contains('{') {
                let name = line.strip_prefix("struct ").unwrap_or("")
                    .split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Struct,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_c_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(header) = line.strip_prefix("#include ") {
                let header = header.trim().trim_matches('"').trim_matches('<').trim_matches('>');
                imports.push(Import {
                    source: header.to_string(),
                    names: vec![],
                    location: Self::loc(ln, file),
                });
            }
        }
        imports
    }

    // ── C# ──────────────────────────────────────────────────────────────

    fn parse_csharp_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(rest) = line.strip_prefix("public class ")
                .or_else(|| line.strip_prefix("class "))
            {
                let name = rest.split(':').next().unwrap_or(rest).split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(rest) = line.strip_prefix("public interface ")
                .or_else(|| line.strip_prefix("interface "))
            {
                let name = rest.split(':').next().unwrap_or(rest).split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
            if line.contains('(') && line.contains(')') {
                let is_public = line.starts_with("public ")
                    || line.starts_with("private ")
                    || line.starts_with("protected ");
                if is_public || line.starts_with("void ") || line.starts_with("int ")
                    || line.starts_with("string ") || line.starts_with("bool ")
                    || line.starts_with("Task<") || line.starts_with("async ")
                {
                    if let Some(name) = line.split('(').next() {
                        let name = name.split(' ').last().unwrap_or("");
                        if !name.is_empty() && name != "class" && name != "interface" {
                            symbols.push(CodeSymbol {
                                name: name.to_string(),
                                kind: SymbolKind::Method,
                                location: Self::loc(ln, file),
                                visibility: Visibility::Public,
                                signature: line.to_string(),
                            });
                        }
                    }
                }
            }
        }
        symbols
    }

    fn parse_csharp_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(ns) = line.strip_prefix("using ") {
                let ns = ns.trim_end_matches(';').trim();
                imports.push(Import {
                    source: ns.to_string(),
                    names: vec![],
                    location: Self::loc(ln, file),
                });
            }
        }
        imports
    }

    // ── PHP ─────────────────────────────────────────────────────────────

    fn parse_php_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("function ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("class ") {
                let name = name.split('{').next().unwrap_or(name).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("interface ") {
                let name = name.split('{').next().unwrap_or(name).split(' ').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_php_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(ns) = line.strip_prefix("use ") {
                let ns = ns.trim_end_matches(';').trim();
                imports.push(Import {
                    source: ns.to_string(),
                    names: vec![],
                    location: Self::loc(ln, file),
                });
            }
        }
        imports
    }

    // ── Kotlin ──────────────────────────────────────────────────────────

    fn parse_kotlin_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("fun ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("class ") {
                let name = name.split('(').next().unwrap_or(name).split(':').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("interface ") {
                let name = name.split(':').next().unwrap_or(name).trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_kotlin_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(path) = line.strip_prefix("import ") {
                imports.push(Import {
                    source: path.to_string(),
                    names: vec![],
                    location: Self::loc(ln, file),
                });
            }
        }
        imports
    }

    // ── Swift ───────────────────────────────────────────────────────────

    fn parse_swift_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("func ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: if line.starts_with("public ") { Visibility::Public }
                        else if line.starts_with("private ") { Visibility::Private }
                        else { Visibility::Internal },
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("class ") {
                let name = name.split(':').next().unwrap_or(name).split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Class,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("struct ") {
                let name = name.split(':').next().unwrap_or(name).split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Struct,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("enum ") {
                let name = name.split(':').next().unwrap_or(name).split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Enum,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("protocol ") {
                let name = name.split(':').next().unwrap_or(name).split('{').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Interface,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_swift_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(module) = line.strip_prefix("import ") {
                imports.push(Import {
                    source: module.trim().to_string(),
                    names: vec![],
                    location: Self::loc(ln, file),
                });
            }
        }
        imports
    }

    // ── Zig ─────────────────────────────────────────────────────────────

    fn parse_zig_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("fn ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("const ") {
                if let Some(eq_pos) = name.find(" = ") {
                    let name = name[..eq_pos].trim();
                    symbols.push(CodeSymbol {
                        name: name.to_string(),
                        kind: SymbolKind::Variable,
                        location: Self::loc(ln, file),
                        visibility: Visibility::Public,
                        signature: line.to_string(),
                    });
                }
            }
        }
        symbols
    }

    fn parse_zig_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(rest) = line.strip_prefix("const ") {
                if let Some(path_start) = rest.find("@import(\"") {
                    let after = &rest[path_start + 9..];
                    if let Some(end) = after.find('"') {
                        imports.push(Import {
                            source: after[..end].to_string(),
                            names: vec![],
                            location: Self::loc(ln, file),
                        });
                    }
                }
            }
        }
        imports
    }

    // ── Lua ─────────────────────────────────────────────────────────────

    fn parse_lua_symbols(content: &str, file: &str) -> Vec<CodeSymbol> {
        let mut symbols = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(name) = line.strip_prefix("function ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Public,
                    signature: line.to_string(),
                });
            } else if let Some(name) = line.strip_prefix("local function ") {
                let name = name.split('(').next().unwrap_or("").trim();
                symbols.push(CodeSymbol {
                    name: name.to_string(),
                    kind: SymbolKind::Function,
                    location: Self::loc(ln, file),
                    visibility: Visibility::Private,
                    signature: line.to_string(),
                });
            }
        }
        symbols
    }

    fn parse_lua_imports(content: &str, file: &str) -> Vec<Import> {
        let mut imports = Vec::new();
        for (i, line) in content.lines().enumerate() {
            let line = line.trim();
            let ln = i + 1;
            if let Some(rest) = line.strip_prefix("require \"") {
                if let Some(end) = rest.find('"') {
                    imports.push(Import {
                        source: rest[..end].to_string(),
                        names: vec![],
                        location: Self::loc(ln, file),
                    });
                }
            } else if let Some(rest) = line.strip_prefix("require(") {
                let after = rest.trim_start().trim_start_matches('"').trim_start_matches('\'');
                let source = after.split('"').next().unwrap_or(after).split('\'').next().unwrap_or("").trim();
                if !source.is_empty() {
                    imports.push(Import {
                        source: source.to_string(),
                        names: vec![],
                        location: Self::loc(ln, file),
                    });
                }
            }
        }
        imports
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_rust_file() {
        let code = r#"
use std::collections::HashMap;

pub fn hello(name: &str) -> String {
    format!("Hello, {}!", name)
}

fn internal() {}
"#;
        let module = SourceAnalyzer::analyze_file("main.rs", code).unwrap();
        assert_eq!(module.language, Language::Rust);
        assert_eq!(module.symbols.len(), 2);
        assert_eq!(module.symbols[0].name, "hello");
        assert_eq!(module.symbols[0].visibility, Visibility::Public);
        assert_eq!(module.imports.len(), 1);
    }

    #[test]
    fn analyze_python_file() {
        let code = r#"
import os
import sys
from datetime import datetime

def greet(name):
    print(f"Hello, {name}")

class Person:
    pass
"#;
        let module = SourceAnalyzer::analyze_file("main.py", code).unwrap();
        assert_eq!(module.symbols.len(), 2);
        assert_eq!(module.imports.len(), 3);
    }

    #[test]
    fn analyze_go_file() {
        let code = r#"
package main

import (
    "fmt"
    "os"
)

func main() {
    fmt.Println("hello")
}

func helper() string {
    return "x"
}
"#;
        let module = SourceAnalyzer::analyze_file("main.go", code).unwrap();
        assert_eq!(module.symbols.len(), 2);
        assert_eq!(module.imports.len(), 2);
    }

    #[test]
    fn analyze_js_file() {
        let code = r#"
import { something } from "./module";
const express = require("express");

function handler(req, res) {
    return "ok";
}

class Service {
    run() {}
}
"#;
        let module = SourceAnalyzer::analyze_file("app.js", code).unwrap();
        assert_eq!(module.symbols.len(), 3); // function + class + const variable
        assert_eq!(module.imports.len(), 2);
    }

    #[test]
    fn analyze_java_file() {
        let code = r#"
package com.example;
import com.example.model.User;

public class HelloService {
    public String greet(String name) {
        return "Hello";
    }
    private void helper() {}
}
"#;
        let module = SourceAnalyzer::analyze_file("HelloService.java", code).unwrap();
        assert_eq!(module.language, Language::Java);
        assert_eq!(module.symbols.len(), 3); // class + 2 methods
    }

    #[test]
    fn unknown_file_returns_none() {
        let result = SourceAnalyzer::analyze_file("readme.txt", "some text");
        assert!(result.is_none());
    }

    #[test]
    fn rust_pub_struct_detected() {
        let code = "pub struct Config {\n    pub name: String,\n}";
        let module = SourceAnalyzer::analyze_file("lib.rs", code).unwrap();
        assert_eq!(module.symbols.len(), 1);
        assert_eq!(module.symbols[0].name, "Config");
        assert_eq!(module.symbols[0].kind, SymbolKind::Struct);
    }

    #[test]
    fn c_functions_detected() {
        let code = r#"
#include <stdio.h>
#include "helper.h"

int main(int argc, char **argv) {
    return 0;
}

void helper() {}
"#;
        let module = SourceAnalyzer::analyze_file("main.c", code).unwrap();
        assert!(module.symbols.len() >= 1);
        assert_eq!(module.imports.len(), 2);
    }

    #[test]
    fn swift_symbols_detected() {
        let code = r#"
import Foundation

class UserManager {
    func login() {}
}

struct Config {
    let version: String
}
"#;
        let module = SourceAnalyzer::analyze_file("app.swift", code).unwrap();
        assert_eq!(module.symbols.len(), 3); // class + method + struct
    }

    #[test]
    fn wasm_returns_empty_symbols() {
        let code = "\0asm\x01\x00\x00\x00";
        let module = SourceAnalyzer::analyze_file("module.wasm", code).unwrap();
        assert_eq!(module.language, Language::Wasm);
        assert!(module.symbols.is_empty());
    }
}
