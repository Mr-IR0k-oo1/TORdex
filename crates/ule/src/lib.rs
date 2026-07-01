pub mod frontend;

use std::collections::HashMap;
use tordex_tdxl::kir::KirProgram;

/// Error type for the ULE.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no frontend registered for language `{0}`")]
    NoFrontend(String),
    #[error("no frontend supports extension `.{0}`")]
    NoFrontendForExtension(String),
    #[error("frontend error: {0}")]
    Frontend(String),
}

/// A language frontend converts source code in a specific language into KIR.
pub trait LanguageFrontend: std::fmt::Debug {
    /// Human-readable name (e.g. "Python", "Rust").
    fn name(&self) -> &str;
    /// File extensions this frontend handles (e.g. `["py"]`).
    fn extensions(&self) -> &[&str];
    /// Parse source and produce a KIR program.
    fn compile(&self, source: &str) -> Result<KirProgram, Error>;
}

/// Universal Language Engine — registers frontends and dispatches compilation.
#[derive(Debug, Default)]
pub struct ULEngine {
    frontends_by_lang: HashMap<String, Box<dyn LanguageFrontend>>,
    ext_to_lang: HashMap<String, String>,
}

impl ULEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a language frontend.
    pub fn register(&mut self, frontend: Box<dyn LanguageFrontend>) {
        let name = frontend.name().to_lowercase();
        for ext in frontend.extensions() {
            self.ext_to_lang.insert(ext.to_string(), name.clone());
        }
        self.frontends_by_lang.insert(name.clone(), frontend);
    }

    /// Compile source code by language name.
    pub fn compile(&self, source: &str, language: &str) -> Result<KirProgram, Error> {
        let key = language.to_lowercase();
        self.frontends_by_lang
            .get(&key)
            .ok_or_else(|| Error::NoFrontend(key))
            .and_then(|f| f.compile(source))
    }

    /// Compile source code by filename (detects language from extension).
    pub fn compile_file(&self, source: &str, filename: &str) -> Result<KirProgram, Error> {
        let ext = filename
            .rsplit('.')
            .next()
            .unwrap_or("")
            .to_lowercase();
        let lang_name = self
            .ext_to_lang
            .get(&ext)
            .ok_or_else(|| Error::NoFrontendForExtension(ext))?;
        self.compile(source, lang_name)
    }

    /// Return all registered frontend names.
    pub fn frontends(&self) -> Vec<&str> {
        self.frontends_by_lang.keys().map(|s| s.as_str()).collect()
    }
}

/// Build a ULEngine with all built-in frontends registered.
pub fn engine_with_all_frontends() -> ULEngine {
    let mut engine = ULEngine::new();
    engine.register(Box::new(frontend::python::PythonFrontend::default()));
    engine.register(Box::new(frontend::javascript::JavaScriptFrontend::default()));
    engine.register(Box::new(frontend::rust::RustFrontend::default()));
    engine.register(Box::new(frontend::java::JavaFrontend::default()));
    engine.register(Box::new(frontend::go::GoFrontend::default()));
    engine
}
