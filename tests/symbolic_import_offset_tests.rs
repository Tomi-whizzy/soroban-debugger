use soroban_debugger::analyzer::symbolic::SymbolicAnalyzer;

fn push_u32_leb(mut value: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn push_name(name: &str, out: &mut Vec<u8>) {
    push_u32_leb(name.len() as u32, out);
    out.extend_from_slice(name.as_bytes());
}

fn append_section(module: &mut Vec<u8>, section_id: u8, section_data: &[u8]) {
    module.push(section_id);
    push_u32_leb(section_data.len() as u32, module);
    module.extend_from_slice(section_data);
}

fn wasm_with_import_and_exported_local() -> Vec<u8> {
    let mut module = Vec::new();
    module.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d]);
    module.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);

    // Type section: type 0 = () -> (), type 1 = (i64, i64) -> ()
    let mut types = Vec::new();
    push_u32_leb(2, &mut types);
    types.push(0x60);
    push_u32_leb(0, &mut types);
    push_u32_leb(0, &mut types);
    types.push(0x60);
    push_u32_leb(2, &mut types);
    types.push(0x7e);
    types.push(0x7e);
    push_u32_leb(0, &mut types);
    append_section(&mut module, 1, &types);

    // Import section: one imported function using type 0
    let mut imports = Vec::new();
    push_u32_leb(1, &mut imports);
    push_name("env", &mut imports);
    push_name("imported", &mut imports);
    imports.push(0x00);
    push_u32_leb(0, &mut imports);
    append_section(&mut module, 2, &imports);

    // Function section: one local function using type 1
    let mut functions = Vec::new();
    push_u32_leb(1, &mut functions);
    push_u32_leb(1, &mut functions);
    append_section(&mut module, 3, &functions);

    // Export section: export local function at global index 1
    let mut exports = Vec::new();
    push_u32_leb(1, &mut exports);
    push_name("entry", &mut exports);
    exports.push(0x00);
    push_u32_leb(1, &mut exports);
    append_section(&mut module, 7, &exports);

    // Code section: one empty function body
    let mut code = Vec::new();
    push_u32_leb(1, &mut code);
    let body = vec![0x00, 0x0b];
    push_u32_leb(body.len() as u32, &mut code);
    code.extend_from_slice(&body);
    append_section(&mut module, 10, &code);

    module
}

#[test]
fn symbolic_analyzer_uses_correct_arg_count_with_imported_functions() {
    let analyzer = SymbolicAnalyzer::new();
    let wasm = wasm_with_import_and_exported_local();

    let report = analyzer
        .analyze(&wasm, "entry")
        .expect("symbolic analysis should complete");

    // Two arguments should generate 6x6 combinations.
    assert_eq!(report.paths_explored, 36);
    assert!(report.paths.iter().any(|p| p.inputs == "[0, 0]"));
    assert!(report.paths.iter().any(|p| p.inputs == "[42, -1]"));
}
