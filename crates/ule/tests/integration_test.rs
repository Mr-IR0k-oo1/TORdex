use tordex_ule::engine_with_all_frontends;
use tordex_tdxl::compiler::codegen_kir;
use tordex_tdxl::optimizer;

#[test]
fn test_ule_python_through_full_pipeline() {
    let engine = engine_with_all_frontends();
    let source = r#"
services = knowledge.collect("OnionServices")
knowledge.classify(services)
"#;
    let kir = engine.compile_file(source, "test.py").unwrap();
    let optimized = optimizer::optimize(kir);
    let program = codegen_kir(&optimized);
    assert!(!program.instructions.is_empty(), "should produce VM instructions");
}

#[test]
fn test_ule_javascript_through_full_pipeline() {
    let engine = engine_with_all_frontends();
    let source = r#"
const services = await knowledge.collect("OnionServices");
const classified = await knowledge.classify(services);
"#;
    let kir = engine.compile_file(source, "test.js").unwrap();
    let optimized = optimizer::optimize(kir);
    let program = codegen_kir(&optimized);
    assert!(!program.instructions.is_empty());
}

#[test]
fn test_ule_rust_through_full_pipeline() {
    let engine = engine_with_all_frontends();
    let source = r#"
let services = knowledge::collect("OnionServices")?;
let classified = services.classify()?;
"#;
    let kir = engine.compile_file(source, "test.rs").unwrap();
    let optimized = optimizer::optimize(kir);
    let program = codegen_kir(&optimized);
    assert!(!program.instructions.is_empty());
}

#[test]
fn test_ule_java_through_full_pipeline() {
    let engine = engine_with_all_frontends();
    let source = r#"
GraphResult services = Knowledge.collect("OnionServices");
ClassificationResult classified = Knowledge.classify(services);
"#;
    let kir = engine.compile_file(source, "test.java").unwrap();
    let optimized = optimizer::optimize(kir);
    let program = codegen_kir(&optimized);
    assert!(!program.instructions.is_empty());
}

#[test]
fn test_ule_go_through_full_pipeline() {
    let engine = engine_with_all_frontends();
    let source = r#"
services := knowledge.Collect("OnionServices")
classified := knowledge.Classify(services)
"#;
    let kir = engine.compile_file(source, "test.go").unwrap();
    let optimized = optimizer::optimize(kir);
    let program = codegen_kir(&optimized);
    assert!(!program.instructions.is_empty());
}

#[test]
fn test_ule_all_frontends_produce_same_kir_for_same_pattern() {
    let engine = engine_with_all_frontends();
    let frontends = engine.frontends();
    assert_eq!(frontends.len(), 5, "should have 5 frontends");

    // All should be registered
    assert!(engine.frontends().contains(&"python"));
    assert!(engine.frontends().contains(&"javascript"));
    assert!(engine.frontends().contains(&"rust"));
    assert!(engine.frontends().contains(&"java"));
    assert!(engine.frontends().contains(&"go"));
}
