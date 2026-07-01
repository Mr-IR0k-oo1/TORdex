/// Example: compile code from 5 languages through the full KIR pipeline.
use tordex_ule::engine_with_all_frontends;
use tordex_tdxl::compiler::codegen_kir;
use tordex_tdxl::optimizer;

fn main() {
    let engine = engine_with_all_frontends();

    let samples: Vec<(&str, &str, &str)> = vec![
        ("Python", "example.py", r#"
services = knowledge.collect("OnionServices")
knowledge.classify(services)
"#),
        ("JavaScript", "example.js", r#"
const services = await knowledge.collect("OnionServices");
const classified = await knowledge.classify(services);
"#),
        ("Rust", "example.rs", r#"
let services = knowledge::collect("OnionServices")?;
let classified = services.classify()?;
"#),
        ("Java", "example.java", r#"
GraphResult services = Knowledge.collect("OnionServices");
ClassificationResult classified = Knowledge.classify(services);
"#),
        ("Go", "example.go", r#"
services := knowledge.Collect("OnionServices")
classified := knowledge.Classify(services)
"#),
    ];

    for (lang, filename, source) in &samples {
        let kir = engine.compile_file(source, filename)
            .expect(&format!("{} frontend failed", lang));
        let optimized = optimizer::optimize(kir);
        let _program = codegen_kir(&optimized);
        println!("  {:12} ✓ compiled ({} KIR ops → VM bytecode)", lang, optimized.ops.len());
    }
}
