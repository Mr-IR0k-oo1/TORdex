use serde::{Deserialize, Serialize};

/// Supported programming languages for repository intelligence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    Python,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    JavaScript,
    TypeScript,
    PHP,
    Kotlin,
    Swift,
    Zig,
    Lua,
    Wasm,
}

impl Language {
    /// Detect language from a filename/path.
    #[must_use]
    pub fn from_filename(path: &str) -> Option<Self> {
        let lower = path.to_lowercase();
        if lower.ends_with(".rs") {
            return Some(Self::Rust);
        }
        if lower.ends_with(".py") || lower.ends_with(".pyw") {
            return Some(Self::Python);
        }
        if lower.ends_with(".go") {
            return Some(Self::Go);
        }
        if lower.ends_with(".java") {
            return Some(Self::Java);
        }
        if lower.ends_with(".c") || lower.ends_with(".h") {
            return Some(Self::C);
        }
        if lower.ends_with(".cpp") || lower.ends_with(".cxx") || lower.ends_with(".cc")
            || lower.ends_with(".hpp") || lower.ends_with(".hxx")
        {
            return Some(Self::Cpp);
        }
        if lower.ends_with(".cs") {
            return Some(Self::CSharp);
        }
        if lower.ends_with(".js") || lower.ends_with(".mjs") {
            return Some(Self::JavaScript);
        }
        if lower.ends_with(".ts") || lower.ends_with(".tsx") {
            return Some(Self::TypeScript);
        }
        if lower.ends_with(".php") {
            return Some(Self::PHP);
        }
        if lower.ends_with(".kt") || lower.ends_with(".kts") {
            return Some(Self::Kotlin);
        }
        if lower.ends_with(".swift") {
            return Some(Self::Swift);
        }
        if lower.ends_with(".zig") {
            return Some(Self::Zig);
        }
        if lower.ends_with(".lua") {
            return Some(Self::Lua);
        }
        if lower.ends_with(".wasm") {
            return Some(Self::Wasm);
        }
        None
    }

    /// Detect language from source content (keyword-based heuristic).
    #[must_use]
    pub fn from_content(content: &str) -> Option<Self> {
        let shebang = content.lines().next().unwrap_or("");
        if shebang.starts_with("#!/usr/bin/env python")
            || shebang.starts_with("#!/usr/bin/python")
            || shebang.contains("python")
        {
            return Some(Self::Python);
        }
        if shebang.starts_with("#!/usr/bin/env node") || shebang.contains("node") {
            return Some(Self::JavaScript);
        }
        if shebang.starts_with("#!/usr/bin/env lua") || shebang.contains("lua") {
            return Some(Self::Lua);
        }

        // Check for Rust
        if content.contains("fn main(") || content.contains("let mut ") {
            return Some(Self::Rust);
        }
        // Go
        if content.contains("package main") && content.contains("func main(") {
            return Some(Self::Go);
        }
        // Python
        if content.contains("def ") || content.contains("import ") {
            return Some(Self::Python);
        }
        // Java
        if content.contains("public class ") || content.contains("public static void main") {
            return Some(Self::Java);
        }
        // Lua
        if content.contains("function ") && content.contains("end") {
            return Some(Self::Lua);
        }
        // TypeScript type annotations
        if (content.contains(": string") || content.contains(": number")
            || content.contains("interface "))
            && (content.contains("const ") || content.contains("let "))
        {
            return Some(Self::TypeScript);
        }
        None
    }

    /// Human-readable name.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::Python => "Python",
            Self::Go => "Go",
            Self::Java => "Java",
            Self::C => "C",
            Self::Cpp => "C++",
            Self::CSharp => "C#",
            Self::JavaScript => "JavaScript",
            Self::TypeScript => "TypeScript",
            Self::PHP => "PHP",
            Self::Kotlin => "Kotlin",
            Self::Swift => "Swift",
            Self::Zig => "Zig",
            Self::Lua => "Lua",
            Self::Wasm => "WASM",
        }
    }

    /// Extensions associated with this language.
    #[must_use]
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::Python => &["py", "pyw"],
            Self::Go => &["go"],
            Self::Java => &["java"],
            Self::C => &["c", "h"],
            Self::Cpp => &["cpp", "cxx", "cc", "hpp", "hxx"],
            Self::CSharp => &["cs"],
            Self::JavaScript => &["js", "mjs"],
            Self::TypeScript => &["ts", "tsx"],
            Self::PHP => &["php"],
            Self::Kotlin => &["kt", "kts"],
            Self::Swift => &["swift"],
            Self::Zig => &["zig"],
            Self::Lua => &["lua"],
            Self::Wasm => &["wasm"],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_rs() {
        assert_eq!(Language::from_filename("main.rs"), Some(Language::Rust));
        assert_eq!(Language::from_filename("lib.rs"), Some(Language::Rust));
    }

    #[test]
    fn detect_py() {
        assert_eq!(Language::from_filename("script.py"), Some(Language::Python));
        assert_eq!(Language::from_filename("script.pyw"), Some(Language::Python));
    }

    #[test]
    fn detect_other_languages() {
        assert_eq!(Language::from_filename("main.go"), Some(Language::Go));
        assert_eq!(Language::from_filename("App.java"), Some(Language::Java));
        assert_eq!(Language::from_filename("main.c"), Some(Language::C));
        assert_eq!(Language::from_filename("main.cpp"), Some(Language::Cpp));
        assert_eq!(Language::from_filename("program.cs"), Some(Language::CSharp));
        assert_eq!(Language::from_filename("app.js"), Some(Language::JavaScript));
        assert_eq!(Language::from_filename("app.ts"), Some(Language::TypeScript));
        assert_eq!(Language::from_filename("index.php"), Some(Language::PHP));
        assert_eq!(Language::from_filename("main.kt"), Some(Language::Kotlin));
        assert_eq!(Language::from_filename("main.swift"), Some(Language::Swift));
        assert_eq!(Language::from_filename("main.zig"), Some(Language::Zig));
        assert_eq!(Language::from_filename("script.lua"), Some(Language::Lua));
        assert_eq!(Language::from_filename("module.wasm"), Some(Language::Wasm));
    }

    #[test]
    fn detect_from_content_rust() {
        let code = "fn main() {\n    let mut x = 1;\n}";
        assert_eq!(Language::from_content(code), Some(Language::Rust));
    }

    #[test]
    fn detect_from_content_python() {
        let code = "def hello():\n    print('hi')\n";
        assert_eq!(Language::from_content(code), Some(Language::Python));
    }

    #[test]
    fn detect_from_content_go() {
        let code = "package main\nfunc main() {\n}";
        assert_eq!(Language::from_content(code), Some(Language::Go));
    }

    #[test]
    fn unknown_extension() {
        assert!(Language::from_filename("file.txt").is_none());
    }

    #[test]
    fn name_is_human_readable() {
        assert_eq!(Language::Cpp.name(), "C++");
        assert_eq!(Language::CSharp.name(), "C#");
        assert_eq!(Language::JavaScript.name(), "JavaScript");
    }
}
