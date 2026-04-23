use crate::{
    cli::args::OutputFormat,
    utils::wasm::{
        extract_contract_metadata, extract_wasm_artifact_metadata, get_module_info,
        parse_function_signatures, parse_functions,
    },
    InspectArgs, Result,
};
use colored::Colorize;
use serde::Serialize;
use std::path::Path;

const BAR_WIDTH: usize = 54;

pub fn run(args: &InspectArgs) -> Result<()> {
    let wasm_file = crate::utils::wasm::load_wasm(&args.contract)
        .map_err(|e| anyhow::anyhow!("Cannot read WASM file '{}': {e}", args.contract.display()))?;
    let wasm_bytes = wasm_file.bytes;
    let wasm_hash = wasm_file.sha256_hash;

    if let Some(expected) = &args.expected_hash {
        if expected.to_lowercase() != wasm_hash {
            return Err(crate::DebuggerError::ChecksumMismatch(
                expected.clone(),
                wasm_hash.clone(),
            )
            .into());
        }
    }

    if args.functions {
        output_functions(&args.contract, &wasm_bytes, args.format)
    } else if args.format == OutputFormat::Json {
        print_json_report(&args.contract, &wasm_bytes)
    } else {
        crate::logging::log_display("", crate::logging::LogLevel::Info);
        print_report(&args.contract, &wasm_bytes)
    }
}

#[derive(Serialize)]
struct FunctionParam {
    name: String,
    #[serde(rename = "type")]
    type_name: String,
}

#[derive(Serialize)]
struct FunctionSignatureJson {
    name: String,
    params: Vec<FunctionParam>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_type: Option<String>,
    has_source_debug: bool,
}

#[derive(Serialize)]
struct FunctionListing {
    file: String,
    exported_functions: Vec<FunctionSignatureJson>,
}

#[derive(Serialize)]
struct SourceMapReport {
    mappings_count: usize,
    diagnostics: Vec<crate::debugger::source_map::SourceMapDiagnostic>,
    units_processed: usize,
    units_with_line_program: usize,
    coverage_ratio: f64,
}

#[derive(Serialize)]
struct FullReport {
    file: String,
    size_bytes: usize,
    module_info: crate::utils::wasm::ModuleInfo,
    functions: Vec<String>,
    signatures: Vec<crate::utils::wasm::FunctionSignature>,
    metadata: crate::utils::wasm::ContractMetadata,
    artifact_metadata: crate::utils::wasm::WasmArtifactMetadata,
    source_map: SourceMapReport,
}

fn output_functions(path: &Path, wasm_bytes: &[u8], format: OutputFormat) -> Result<()> {
    let signatures = parse_function_signatures(wasm_bytes)?;
    let mut source_map = crate::debugger::source_map::SourceMap::new();
    let _ = source_map.load(wasm_bytes);

    match format {
        OutputFormat::Json => {
            let functions = signatures
                .into_iter()
                .map(|sig| {
                    let has_debug = source_map.function_has_source_mapped(wasm_bytes, &sig.name);
                    FunctionSignatureJson {
                        name: sig.name,
                        params: sig
                            .params
                            .into_iter()
                            .map(|p| FunctionParam {
                                name: p.name,
                                type_name: p.type_name,
                            })
                            .collect(),
                        return_type: sig.return_type.filter(|r| r != "Void"),
                        has_source_debug: has_debug,
                    }
                })
                .collect();

            let listing = FunctionListing {
                file: path.display().to_string(),
                exported_functions: functions,
            };

            println!("{}", serde_json::to_string_pretty(&listing)?);
            Ok(())
        }
        OutputFormat::Pretty => print_pretty_functions(&signatures, wasm_bytes, &source_map),
    }
}

fn print_pretty_functions(
    signatures: &[crate::utils::wasm::FunctionSignature],
    wasm_bytes: &[u8],
    source_map: &crate::debugger::source_map::SourceMap,
) -> Result<()> {
    if signatures.is_empty() {
        let functions = parse_functions(wasm_bytes).unwrap_or_default();
        if functions.is_empty() {
            println!("(no contractspecv0 section found and no functions exported)");
        } else {
            println!("(no contractspecv0 section found)\nBare functions exported:");
            for f in functions {
                let has_debug = source_map.function_has_source_mapped(wasm_bytes, &f);
                let debug_str = if has_debug {
                    " (Source/Debug: Yes)"
                } else {
                    ""
                };
                println!("  {}{}", f, debug_str);
            }
        }
    } else {
        let name_w = std::cmp::max(
            signatures.iter().map(|s| s.name.len()).max().unwrap_or(8),
            8,
        );
        println!(
            "{:<name_w$}  {:<45}  Source/Debug",
            "Function",
            "Signature",
            name_w = name_w
        );
        println!(
            "{}  {}  {}",
            "─".repeat(name_w),
            "─".repeat(45),
            "─".repeat(12)
        );

        for sig in signatures {
            let params = sig
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, p.type_name))
                .collect::<Vec<_>>()
                .join(", ");
            let ret = sig
                .return_type
                .as_ref()
                .filter(|t| t != "Void")
                .map(|t| format!(" -> {t}"))
                .unwrap_or_default();

            let sig_str = format!("({}){}", params, ret);
            let has_debug = source_map.function_has_source_mapped(wasm_bytes, &sig.name);
            let debug_str = if has_debug { "Yes" } else { "No" };

            println!(
                "{:<name_w$}  {:<45}  {}",
                sig.name,
                sig_str,
                debug_str,
                name_w = name_w
            );
        }
    }
    Ok(())
}

fn print_json_report(path: &Path, wasm_bytes: &[u8]) -> Result<()> {
    let info = get_module_info(wasm_bytes)?;
    let functions = parse_functions(wasm_bytes)?;
    let signatures = parse_function_signatures(wasm_bytes)?;
    let metadata = extract_contract_metadata(wasm_bytes)?;
    let artifact_metadata = extract_wasm_artifact_metadata(wasm_bytes)?;

    let mut source_map = crate::debugger::source_map::SourceMap::new();
    let _ = source_map.load(wasm_bytes);

    let report = FullReport {
        file: path.display().to_string(),
        size_bytes: wasm_bytes.len(),
        module_info: info,
        functions,
        signatures,
        metadata,
        artifact_metadata,
        source_map: SourceMapReport {
            mappings_count: source_map.len(),
            diagnostics: source_map.diagnostics.clone(),
            units_processed: source_map.units_processed,
            units_with_line_program: source_map.units_with_line_program,
            coverage_ratio: if source_map.units_processed > 0 {
                source_map.units_with_line_program as f64 / source_map.units_processed as f64
            } else {
                0.0
            },
        },
    };

    crate::logging::log_display(
        serde_json::to_string_pretty(&report)?,
        crate::logging::LogLevel::Info,
    );
    Ok(())
}

fn print_report(path: &Path, wasm_bytes: &[u8]) -> Result<()> {
    let info = get_module_info(wasm_bytes)?;
    let signatures = parse_function_signatures(wasm_bytes)?;
    let metadata = extract_contract_metadata(wasm_bytes)?;
    let artifact_metadata = extract_wasm_artifact_metadata(wasm_bytes)?;

    let separator = "═".repeat(BAR_WIDTH);
    let size_kb = wasm_bytes.len() as f64 / 1024.0;

    log_both(&separator);
    log_both(&format!("  {}", "Soroban Contract Inspector".bold().cyan()));
    log_both(&separator);
    log_both("");
    log_both(&format!(
        "  File : {}",
        path.display().to_string().bright_white()
    ));
    log_both(&format!(
        "  Size : {} ({:.2} KB)\n",
        format!("{} bytes", wasm_bytes.len()).bright_white(),
        size_kb
    ));

    print_section("Module Statistics", || {
        log_both(&format!(
            "  Types      : {}",
            info.type_count.to_string().bright_white()
        ));
        log_both(&format!(
            "  Functions  : {}",
            info.function_count.to_string().bright_white()
        ));
        log_both(&format!(
            "  Exports    : {}\n",
            info.export_count.to_string().bright_white()
        ));
    });

    print_section("WASM Section Breakdown", || {
        log_both(&format!(
            "  {:<20} | {:>10} | {:>6}",
            "Section", "Size", "Total%"
        ));
        log_both(&format!(
            "  {}|{}|{}",
            "─".repeat(21),
            "─".repeat(12),
            "─".repeat(8)
        ));

        for section in &info.sections {
            let percentage = (section.size as f64 / info.total_size as f64) * 100.0;
            let row = format!(
                "  {:<20} | {:>10} | {:>5.1}%",
                section.name,
                format!("{} B", section.size),
                percentage
            );

            let styled = if section.size > 50 * 1024 || percentage > 50.0 {
                row.red().bold().to_string()
            } else if section.size > 10 * 1024 {
                row.yellow().to_string()
            } else {
                row.bright_white().to_string()
            };
            log_both(&styled);
        }
    });

    print_section("Exported Functions", || {
        if signatures.is_empty() {
            let functions = parse_functions(wasm_bytes).unwrap_or_default();
            if functions.is_empty() {
                log_both("  (no contractspecv0 section found and no functions exported)");
            } else {
                log_both("  (no contractspecv0 section found)");
                log_both("  Bare functions exported:");
                for f in functions {
                    log_both(&format!("    {}", f));
                }
            }
        } else {
            let name_w = signatures.iter().map(|s| s.name.len()).max().unwrap_or(8);
            log_both(&format!(
                "  {:<name_w$}  Signature",
                "Function",
                name_w = name_w
            ));
            log_both(&format!(
                "  {}  {}",
                "─".repeat(name_w),
                "─".repeat(BAR_WIDTH - name_w - 4)
            ));

            for sig in &signatures {
                let params = sig
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, p.type_name))
                    .collect::<Vec<_>>()
                    .join(", ");
                let ret = sig
                    .return_type
                    .as_ref()
                    .filter(|t| t != "Void")
                    .map(|t| format!(" -> {t}"))
                    .unwrap_or_default();
                log_both(&format!(
                    "  {:<name_w$}  ({}){ret}",
                    sig.name,
                    params,
                    name_w = name_w
                ));
            }
        }
    });

    print_section("Contract Metadata", || {
        if metadata.is_empty() {
            log_both("  ⚠  No metadata section embedded in this contract.");
        } else {
            log_both_if_some("Contract Version", &metadata.contract_version);
            log_both_if_some("SDK Version", &metadata.sdk_version);
            log_both_if_some("Build Date", &metadata.build_date);
            log_both_if_some("Author / Org", &metadata.author);
            log_both_if_some("Description", &metadata.description);
            log_both_if_some("Implementation", &metadata.implementation);
        }
    });

    print_section("Artifact Metadata", || {
        log_both(&format!(
            "  Build Profile Hint : {}",
            artifact_metadata.build_profile_hint
        ));
        log_both(&format!(
            "  Optimization Hint  : {}",
            artifact_metadata.optimization_hint
        ));
        log_both(&format!(
            "  Name Section       : {}",
            if artifact_metadata.name_section_present {
                "present"
            } else {
                "absent"
            }
        ));
        log_both(&format!(
            "  DWARF Sections     : {}",
            if artifact_metadata.has_debug_sections {
                if artifact_metadata.debug_sections.is_empty() {
                    "present".to_string()
                } else {
                    format!(
                        "{} ({} bytes)",
                        artifact_metadata.debug_sections.join(", "),
                        artifact_metadata.debug_section_bytes
                    )
                }
            } else {
                "absent".to_string()
            }
        ));
        log_both_if_some("Module Name", &artifact_metadata.module_name);
        if !artifact_metadata.package_hints.is_empty() {
            log_both("  Package Hints:");
            for hint in &artifact_metadata.package_hints {
                log_both(&format!("    - {}", hint));
            }
        }
    });

    section_header("Contract Metadata");
    if metadata.is_empty() {
        log_both("  ⚠  No metadata section embedded in this contract.");
    } else {
        log_both_if_some("Contract Version", &metadata.contract_version);
        log_both_if_some("SDK Version", &metadata.sdk_version);
        log_both_if_some("Build Date", &metadata.build_date);
        log_both_if_some("Author / Org", &metadata.author);
        log_both_if_some("Description", &metadata.description);
        log_both_if_some("Implementation", &metadata.implementation);
    }
    log_both("");

    section_header("Source Map (DWARF)");
    let mut source_map = crate::debugger::source_map::SourceMap::new();
    let _ = source_map.load(wasm_bytes);

    if source_map.is_empty() && source_map.diagnostics.is_empty() {
        log_both("  No DWARF debug information found in this contract.");
    } else {
        log_both(&format!(
            "  Mapped Executable Lines : {}",
            source_map.len().to_string().bright_white()
        ));
        log_both(&format!(
            "  DWARF Units Processed   : {}",
            source_map.units_processed.to_string().bright_white()
        ));
        log_both(&format!(
            "  Units with Mappings     : {}",
            source_map.units_with_line_program.to_string().bright_white()
        ));

        let coverage = if source_map.units_processed > 0 {
            (source_map.units_with_line_program as f64 / source_map.units_processed as f64) * 100.0
        } else {
            0.0
        };

        let coverage_str = format!("{:.1}%", coverage);
        let styled_coverage = if coverage >= 90.0 {
            coverage_str.green().bold()
        } else if coverage >= 50.0 {
            coverage_str.yellow()
        } else {
            coverage_str.red().bold()
        };

        log_both(&format!("  Mapping Coverage        : {}", styled_coverage));

        if !source_map.diagnostics.is_empty() {
            log_both("");
            log_both(&format!(
                "  {}",
                "⚠ Diagnostics / Warnings:".yellow().bold()
            ));
            for diag in &source_map.diagnostics {
                log_both(&format!("    - {}", diag.message.yellow()));
            }
        }
    }

    log_both(&heavy);
    Ok(())
}

fn print_section<F>(title: &str, content: F)
where
    F: FnOnce(),
{
    let fill = BAR_WIDTH.saturating_sub(title.len() + 5);
    log_both(&format!("─── {title} {}", "─".repeat(fill)));
    content();
    log_both("");
}

fn log_both(msg: &str) {
    println!("{}", msg);
    crate::logging::log_display(msg, crate::logging::LogLevel::Info);
}

fn log_both_if_some(label: &str, value: &Option<String>) {
    if let Some(v) = value {
        log_both(&format!("  {label:<20} : {v}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uleb128(mut v: usize) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7F) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                break;
            }
        }
        out
    }

    fn wasm_with_custom_section(name: &str, payload: &[u8]) -> Vec<u8> {
        let mut bytes: Vec<u8> = Vec::new();
        bytes.extend_from_slice(&[0x00, 0x61, 0x73, 0x6d]);
        bytes.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        bytes.push(0x00);

        let mut section = Vec::new();
        section.extend_from_slice(&uleb128(name.len()));
        section.extend_from_slice(name.as_bytes());
        section.extend_from_slice(payload);

        bytes.extend_from_slice(&uleb128(section.len()));
        bytes.extend_from_slice(&section);
        bytes
    }

    fn bare_wasm() -> Vec<u8> {
        vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]
    }

    #[test]
    fn report_on_metadata_absent_wasm_succeeds() {
        assert!(print_report(Path::new("test.wasm"), &bare_wasm()).is_ok());
    }

    #[test]
    fn report_on_metadata_present_wasm_succeeds() {
        let json = r#"{"contract_version":"2.0.0","sdk_version":"22.0.0","author":"Acme Corp"}"#;
        let wasm = wasm_with_custom_section("contractmeta", json.as_bytes());
        assert!(print_report(Path::new("test.wasm"), &wasm).is_ok());
    }

    #[test]
    fn report_on_partial_metadata_succeeds() {
        let json = r#"{"contract_version":"0.1.0"}"#;
        let wasm = wasm_with_custom_section("contractmeta", json.as_bytes());
        assert!(print_report(Path::new("partial.wasm"), &wasm).is_ok());
    }

    #[test]
    fn output_functions_json_on_metadata_absent_succeeds() {
        assert!(output_functions(Path::new("test.wasm"), &bare_wasm(), OutputFormat::Json).is_ok());
    }

    #[test]
    fn output_functions_pretty_on_metadata_absent_succeeds() {
        assert!(
            output_functions(Path::new("test.wasm"), &bare_wasm(), OutputFormat::Pretty).is_ok()
        );
    }

    #[test]
    fn function_listing_serializes_to_valid_json() {
        let listing = FunctionListing {
            file: "test.wasm".to_string(),
            exported_functions: vec![
                FunctionSignatureJson {
                    name: "initialize".to_string(),
                    params: vec![FunctionParam {
                        name: "admin".to_string(),
                        type_name: "Address".to_string(),
                    }],
                    return_type: None,
                },
                FunctionSignatureJson {
                    name: "get_value".to_string(),
                    params: vec![],
                    return_type: Some("i64".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&listing).expect("Failed to serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("Invalid JSON");

        assert!(parsed["file"].is_string());
        assert!(parsed["exported_functions"].is_array());
        assert_eq!(parsed["exported_functions"].as_array().unwrap().len(), 2);
    }
}
