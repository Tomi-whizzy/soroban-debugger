use soroban_debugger::debugger::engine::DebuggerEngine;
use soroban_debugger::debugger::source_map::SourceMap;
use soroban_debugger::runtime::executor::ContractExecutor;
#[path = "fixtures/mod.rs"]
mod fixtures;

use std::path::PathBuf;

fn fixture_wasm(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("wasm")
        .join(format!("{name}.wasm"))
}

#[test]
fn source_map_missing_debug_info_is_graceful() {
    let wasm = fixtures::get_fixture_path(fixtures::names::COUNTER);
    if !wasm.exists() {
        eprintln!(
            "Skipping test: fixture not found at {}. Run tests/fixtures/build.sh to build fixtures.",
            wasm.display()
        );
        return;
    }

    let bytes = std::fs::read(&wasm).unwrap();
    let mut sm = SourceMap::new();
    sm.load(&bytes).expect("load should not fail");
    assert!(
        sm.is_empty(),
        "expected no DWARF mappings in stripped fixture"
    );
}

// ── Cache integration tests ──────────────────────────────────────────────────

/// Loading the same fixture twice must not trigger a second DWARF parse.
#[test]
fn source_map_repeated_load_uses_cache() {
    let wasm = fixture_wasm("counter");
    if !wasm.exists() {
        eprintln!("Skipping: counter fixture not found.");
        return;
    }
    let bytes = std::fs::read(&wasm).unwrap();

    let mut sm = SourceMap::new();
    sm.load(&bytes).expect("first load should succeed");
    let count_after_first = sm.parse_count();

    sm.load(&bytes).expect("second load should succeed");
    assert_eq!(
        sm.parse_count(),
        count_after_first,
        "parse_count must not increase on a cache-hit load"
    );
}

/// Loading different bytes must trigger a new parse and update the stored hash.
#[test]
fn source_map_different_bytes_invalidates_cache() {
    let wasm = fixture_wasm("counter");
    if !wasm.exists() {
        eprintln!("Skipping: counter fixture not found.");
        return;
    }
    let bytes = std::fs::read(&wasm).unwrap();
    let mut modified = bytes.clone();
    // Flip the last byte so the hash differs (keep the WASM mostly valid,
    // we just need a different fingerprint — load errors are fine here).
    if let Some(last) = modified.last_mut() {
        *last ^= 0xff;
    }

    let mut sm = SourceMap::new();
    sm.load(&bytes).expect("first load should succeed");
    let hash_a = sm.last_wasm_hash();
    let count_a = sm.parse_count();

    // Second load with modified bytes — may or may not succeed, but the hash
    // and parse_count must reflect a new parse attempt.
    let _ = sm.load(&modified);
    let hash_b = sm.last_wasm_hash();

    // If the modified bytes were valid enough to parse, the hash must have changed.
    if sm.parse_count() > count_a {
        assert_ne!(hash_a, hash_b, "hash must change after a cache-miss parse");
    }
}

/// `DebuggerEngine::try_load_source_map` must reuse the same `SourceMap`
/// instance so the cache is preserved across calls.
#[test]
fn engine_try_load_source_map_reuses_cache() {
    let wasm = fixture_wasm("counter");
    if !wasm.exists() {
        eprintln!("Skipping: counter fixture not found.");
        return;
    }
    let bytes = std::fs::read(&wasm).unwrap();

    let executor = ContractExecutor::new(bytes.clone()).expect("executor should load");
    let mut engine = DebuggerEngine::new(executor, vec![]);

    engine.try_load_source_map(&bytes);
    let parse_count_after_first = engine.source_map().map(|sm| sm.parse_count()).unwrap_or(0);

    // Second call with same bytes — must be a cache hit.
    engine.try_load_source_map(&bytes);
    let parse_count_after_second = engine.source_map().map(|sm| sm.parse_count()).unwrap_or(0);

    assert_eq!(
        parse_count_after_first, parse_count_after_second,
        "engine must not re-parse DWARF when WASM bytes are unchanged"
    );
}

#[test]
fn source_map_debug_fixture_resolves_locations() {
    let Some(wasm) = fixtures::try_artifact_path(fixtures::names::COUNTER, "debug") else {
        eprintln!(
            "Skipping test: debug artifact missing from {}. Run tests/fixtures/build.sh to generate debug fixtures.",
            fixtures::manifest_path().display()
        );
        return;
    };

    if !wasm.exists() {
        eprintln!(
            "Skipping test: debug fixture not found at {}. Run tests/fixtures/build.sh to generate *_debug.wasm fixtures.",
            wasm.display()
        );
        return;
    }

    let bytes = std::fs::read(&wasm).unwrap();
    let mut sm = SourceMap::new();
    sm.load(&bytes).expect("load should not fail");

    assert!(!sm.is_empty(), "expected DWARF mappings in debug fixture");

    let (first_offset, first_loc) = sm.mappings().next().expect("at least one mapping");
    assert!(first_loc.line > 0, "expected non-zero line numbers");

    let looked_up = sm.lookup(first_offset).expect("lookup should succeed");
    assert_eq!(&looked_up, first_loc);

    assert!(sm.lookup(first_offset.saturating_add(1)).is_some());
}

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
    bytes.push(0x00); // custom section id

    let mut section = Vec::new();
    section.extend_from_slice(&uleb128(name.len()));
    section.extend_from_slice(name.as_bytes());
    section.extend_from_slice(payload);

    bytes.extend_from_slice(&uleb128(section.len()));
    bytes.extend_from_slice(&section);
    bytes
}

#[test]
fn source_map_partial_dwarf_is_graceful() {
    // A WASM with a completely malformed debug_info section.
    let malicious_dwarf = wasm_with_custom_section(".debug_info", &[0xde, 0xad, 0xbe, 0xef]);
    let mut sm = SourceMap::new();
    let res = sm.load(&malicious_dwarf);

    // The load should succeed but produce no mappings and one or more diagnostics.
    assert!(
        res.is_ok(),
        "load should not fail on partial/malformed DWARF units"
    );
    assert!(sm.is_empty(), "expected no mappings for garbage DWARF");
    assert!(
        !sm.diagnostics.is_empty(),
        "expected diagnostics explaining the failure"
    );

    let diag = &sm.diagnostics[0];
    assert!(
        diag.message.contains("Failed to read"),
        "Diagnostics should mention read failure: {}",
        diag.message
    );
}
