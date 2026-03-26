//! Example demonstrating the improved unbounded iteration detection
//!
//! This example shows how the enhanced security analyzer can detect
//! storage-driven loops with improved precision and confidence scoring.

use soroban_debugger::analyzer::security::{AnalyzerFilter, SecurityAnalyzer};

fn main() {
    println!("Testing improved unbounded iteration detection...");

    // Create a simple WASM module with storage calls in loops
    let wasm_with_storage_loop = create_wasm_with_storage_loop();

    let analyzer = SecurityAnalyzer::new();
    let filter = AnalyzerFilter::default();
    match analyzer.analyze(&wasm_with_storage_loop, None, None, &filter) {
        Ok(report) => {
            println!(
                "Analysis complete. Found {} security issues.",
                report.findings.len()
            );

            for finding in &report.findings {
                if finding.rule_id == "unbounded-iteration" {
                    println!("🔍 Unbounded Iteration Finding:");
                    println!("  Severity: {:?}", finding.severity);
                    println!("  Description: {}", finding.description);

                    if let Some(confidence) = &finding.confidence {
                        println!("  Confidence: {:.0}%", confidence);
                    }
                    if let Some(rationale) = &finding.rationale {
                        println!("  Rationale: {}", rationale);
                    }

                    println!("  Remediation: {}", finding.remediation);
                    println!();
                }
            }
        }
        Err(e) => {
            eprintln!("Analysis failed: {}", e);
        }
    }
}

fn create_wasm_with_storage_loop() -> Vec<u8> {
    // This is a minimal WASM module that imports a storage function and calls it in a loop
    // For demonstration purposes, we'll create a simple pattern

    let mut module = vec![
        0x00, 0x61, 0x73, 0x6D, // WASM magic
        0x01, 0x00, 0x00, 0x00, // WASM version
    ];

    // Type section (one function type: () -> ())
    module.extend_from_slice(&[0x01, 0x60, 0x00, 0x00]);

    // Import section (import storage_get from env)
    module.extend_from_slice(&[
        0x02, // Import section id
        0x01, // Number of imports
        0x03, // Length of "env"
    ]);
    module.extend_from_slice(b"env");
    module.extend_from_slice(&[0x0B]); // Length of "storage_get"
    module.extend_from_slice(b"storage_get");
    module.extend_from_slice(&[0x00, 0x00]); // Import kind: function, type index 0

    // Function section (one local function)
    module.extend_from_slice(&[0x03, 0x01, 0x00]);

    // Export section (export the function)
    module.extend_from_slice(&[0x07, 0x01]);
    module.extend_from_slice(&[0x09]); // Length of "test_func"
    module.extend_from_slice(b"test_func");
    module.extend_from_slice(&[0x00, 0x01]); // Export kind: function, function index 1

    // Code section (function body with loop and storage call)
    module.extend_from_slice(&[0x0A, 0x01, 0x09]); // Code section, 1 function, body size 9
    module.extend_from_slice(&[
        0x00, // No local variables
        0x03, // Loop instruction
        0x40, // Empty block type
        0x10, 0x00, // Call imported function (storage_get)
        0x0B, // End loop
        0x0B, // End function
    ]);

    module
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unbounded_iteration_detection() {
        let wasm = create_wasm_with_storage_loop();
        let analyzer = SecurityAnalyzer::new();
        let filter = AnalyzerFilter::default();
        let report = analyzer
            .analyze(&wasm, None, None, &filter)
            .expect("Analysis should succeed");

        // Should find the unbounded iteration issue
        let unbounded_findings: Vec<_> = report
            .findings
            .iter()
            .filter(|f| f.rule_id == "unbounded-iteration")
            .collect();

        assert!(
            !unbounded_findings.is_empty(),
            "Should detect unbounded iteration"
        );

        let finding = unbounded_findings[0];
        assert_eq!(
            finding.severity,
            soroban_debugger::analyzer::security::Severity::High
        );

        // Should have confidence metadata
        assert!(finding.confidence.is_some());
        assert!(finding.rationale.is_some());
        assert!(!finding.rationale.as_ref().unwrap().is_empty());
    }
}
