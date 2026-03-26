// ── commands.rs patch ──────────────────────────────────────────────────────
//
// Replace the existing `pub fn profile(args: ProfileArgs) -> Result<()>`
// with this version. The only change is adding `executor.set_timeout(args.timeout)`
// after the executor is created, matching what `run`, `interactive`, and
// `analyze` already do.
//
// DIFF summary:
//   + executor.set_timeout(args.timeout);
//
// All other lines are identical to the original.
// ──────────────────────────────────────────────────────────────────────────

pub fn profile(args: ProfileArgs) -> Result<()> {
    logging::log_display(
        format!("Profiling contract execution: {:?}", args.contract),
        logging::LogLevel::Info,
    );

    let wasm_file = crate::utils::wasm::load_wasm(&args.contract)
        .with_context(|| format!("Failed to read WASM file: {:?}", args.contract))?;
    let wasm_bytes = wasm_file.bytes;
    let wasm_hash = wasm_file.sha256_hash;

    if let Some(expected) = &args.expected_hash {
        if expected.to_lowercase() != wasm_hash {
            return Err((crate::DebuggerError::ChecksumMismatch {
                expected: expected.clone(),
                actual: wasm_hash.clone(),
            })
            .into());
        }
    }

    logging::log_display(
        format!("Contract loaded successfully ({} bytes)", wasm_bytes.len()),
        logging::LogLevel::Info,
    );

    // Parse args (optional)
    let parsed_args = if let Some(args_json) = &args.args {
        Some(parse_args(args_json)?)
    } else {
        None
    };

    // Create executor
    let mut executor = ContractExecutor::new(wasm_bytes)?;

    // Apply timeout — consistent with run, interactive, and analyze.
    // A value of 0 disables the timeout.
    executor.set_timeout(args.timeout);

    // Initial storage (optional)
    if let Some(storage_json) = &args.storage {
        let storage = parse_storage(storage_json)?;
        executor.set_initial_storage(storage)?;
    }

    // Analyze exactly one function (this command focuses on execution hotspots)
    let mut optimizer = crate::profiler::analyzer::GasOptimizer::new(executor);

    logging::log_display(
        format!("\nRunning function: {}", args.function),
        logging::LogLevel::Info,
    );
    if let Some(ref a) = parsed_args {
        logging::log_display(format!("Args: {}", a), logging::LogLevel::Info);
    }

    let _profile = optimizer.analyze_function(&args.function, parsed_args.as_deref())?;

    let contract_path_str = args.contract.to_string_lossy().to_string();
    let report = optimizer.generate_report(&contract_path_str);

    // Hotspot summary first
    logging::log_display(
        format!("\n{}", report.format_hotspots()),
        logging::LogLevel::Info,
    );

    // Then detailed suggestions (markdown format)
    let markdown = optimizer.generate_markdown_report(&report);

    if let Some(output_path) = &args.output {
        fs::write(output_path, &markdown).map_err(|e| {
            DebuggerError::FileError(format!(
                "Failed to write report to {:?}: {}",
                output_path, e
            ))
        })?;
        logging::log_display(
            format!("\nProfile report written to: {:?}", output_path),
            logging::LogLevel::Info,
        );
    } else {
        logging::log_display(format!("\n{}", markdown), logging::LogLevel::Info);
    }

    Ok(())
}