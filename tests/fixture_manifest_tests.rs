#[path = "fixtures/mod.rs"]
mod fixtures;

use sha2::{Digest, Sha256};
use soroban_debugger::utils::wasm;

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[test]
fn fixture_manifest_matches_checked_in_artifacts() {
    let manifest = fixtures::load_manifest();
    assert_eq!(manifest.version, 1, "unexpected fixture manifest version");
    assert!(
        !manifest.fixtures.is_empty(),
        "fixture manifest should describe at least one fixture"
    );

    for fixture in manifest.fixtures {
        assert!(
            fixtures::contract_dir(&fixture.name).exists(),
            "missing contract dir for {}",
            fixture.name
        );
        assert!(
            fixtures::source_path(&fixture.name).exists(),
            "missing lib.rs source path for {}",
            fixture.name
        );

        let release = fixture
            .artifacts
            .get("release")
            .expect("every fixture must define a release artifact");
        let release_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&release.path);
        assert!(
            release_path.exists(),
            "missing release artifact for {} at {}",
            fixture.name,
            release_path.display()
        );

        let bytes = std::fs::read(&release_path).expect("release artifact should be readable");
        assert_eq!(
            sha256_hex(&bytes),
            release.sha256,
            "sha256 mismatch for {}",
            fixture.name
        );

        let mut exports = wasm::parse_functions(&bytes).expect("fixture exports should parse");
        exports.sort();

        let mut expected_exports = fixture.exports.clone();
        expected_exports.sort();

        assert_eq!(
            exports, expected_exports,
            "export list mismatch for {}",
            fixture.name
        );
    }
}
