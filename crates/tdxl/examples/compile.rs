fn main() {
    let source = r#"
FROM OnionServices
MATCH Technology == "WordPress"
WHERE Status == Alive
TRAVERSE LINKS_TO DEPTH 3
SUMMARIZE
"#;

    match tordex_tdxl::compile_program(source) {
        Ok(program) => {
            println!("TDXL → VM Program: {}", program.name);
            println!("Instructions ({}):", program.instructions.len());
            for (i, instr) in program.instructions.iter().enumerate() {
                println!("  {:>4}: {}", i, instr);
            }
            println!("\nConstants ({}):", program.constants.len());
            for (i, c) in program.constants.iter().enumerate() {
                println!("  {:>4}: {}", i, c);
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
}
