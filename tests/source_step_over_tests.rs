//! Smoke test for source-level step-over functionality

use soroban_debugger::debugger::instruction_pointer::StepMode;
use soroban_debugger::debugger::source_map::{SourceLocation, SourceMap};
use soroban_debugger::debugger::{DebugState, Stepper};
use soroban_debugger::runtime::instruction::Instruction;
use std::path::PathBuf;

#[test]
fn test_step_over_source_line() {
    let mut state = DebugState::new();
    let mut stepper = Stepper::new();
    let mut source_map = SourceMap::new();

    // Create a mock source file
    let file_path = PathBuf::from("src/contract.rs");

    // We simulate instructions for two source lines: line 10 and line 11
    let instructions = vec![
        // Source line 10 (instructions 0, 1)
        Instruction::new(0x100, wasmparser::Operator::I32Const { value: 1 }, 0, 0),
        Instruction::new(
            0x104,
            wasmparser::Operator::LocalSet { local_index: 0 },
            0,
            1,
        ),
        // Source line 11 (instructions 2, 3)
        Instruction::new(0x108, wasmparser::Operator::I32Const { value: 2 }, 0, 2),
        Instruction::new(
            0x10c,
            wasmparser::Operator::LocalSet { local_index: 0 },
            0,
            3,
        ),
        // End of function
        Instruction::new(0x110, wasmparser::Operator::Return, 0, 4),
    ];

    // Add source map entries
    source_map.add_mapping(
        0x100,
        SourceLocation {
            file: file_path.clone(),
            line: 10,
            column: Some(1),
        },
    );
    // Same source line but at different instruction
    source_map.add_mapping(
        0x104,
        SourceLocation {
            file: file_path.clone(),
            line: 10,
            column: Some(10),
        },
    );

    // New source line
    source_map.add_mapping(
        0x108,
        SourceLocation {
            file: file_path.clone(),
            line: 11,
            column: Some(1),
        },
    );
    source_map.add_mapping(
        0x10c,
        SourceLocation {
            file: file_path.clone(),
            line: 11,
            column: Some(10),
        },
    );

    // Line 12
    source_map.add_mapping(
        0x110,
        SourceLocation {
            file: file_path.clone(),
            line: 12,
            column: Some(1),
        },
    );

    // Enable instructions and debug
    state.set_instructions(instructions);
    state.enable_instruction_debug();
    stepper.start(StepMode::StepInto, &mut state); // Activate stepper

    // We are currently at instruction 0 (offset 0x100), which corresponds to line 10.
    assert_eq!(state.instruction_pointer().current_index(), 0);
    assert_eq!(
        source_map
            .lookup(state.current_instruction().unwrap().offset)
            .unwrap()
            .line,
        10
    );

    // Call step_over_source_line. It should advance until the source location changes.
    // That means it should skip instruction 1 (offset 0x104, still line 10)
    // and stop at instruction 2 (offset 0x108, line 11).
    let advanced = stepper.step_over_source_line(&mut state, &source_map);

    assert!(advanced);
    assert_eq!(state.instruction_pointer().current_index(), 2);

    let current_offset = state.current_instruction().unwrap().offset;
    assert_eq!(current_offset, 0x108);

    let new_loc = source_map.lookup(current_offset).unwrap();
    assert_eq!(new_loc.line, 11);
}
