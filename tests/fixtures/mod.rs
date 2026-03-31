//! Test fixture utilities for loading pre-compiled WASM contracts.
#![allow(dead_code)]

use serde::Deserialize;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureManifest {
    pub version: u32,
    pub fixtures: Vec<FixtureContract>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureContract {
    pub name: String,
    pub exports: Vec<String>,
    pub source: FixtureSource,
    pub artifacts: BTreeMap<String, FixtureArtifact>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureSource {
    pub contract_dir: String,
    pub lib_rs: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FixtureArtifact {
    pub path: String,
    pub sha256: String,
}

pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

pub fn manifest_path() -> PathBuf {
    fixtures_root().join("manifest.json")
}

pub fn load_manifest() -> FixtureManifest {
    let path = manifest_path();
    let contents = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Failed to read fixture manifest from {}: {}",
            path.display(),
            e
        )
    });
    serde_json::from_str(&contents).unwrap_or_else(|e| {
        panic!(
            "Failed to parse fixture manifest from {}: {}",
            path.display(),
            e
        )
    })
}

pub fn fixture(name: &str) -> FixtureContract {
    load_manifest()
        .fixtures
        .into_iter()
        .find(|fixture| fixture.name == name)
        .unwrap_or_else(|| {
            panic!(
                "Fixture '{}' not found in {}",
                name,
                manifest_path().display()
            )
        })
}

pub fn artifact_path(name: &str, profile: &str) -> PathBuf {
    try_artifact_path(name, profile).unwrap_or_else(|| {
        panic!(
            "Fixture '{}' does not define a '{}' artifact in {}",
            name,
            profile,
            manifest_path().display()
        )
    })
}

pub fn try_artifact_path(name: &str, profile: &str) -> Option<PathBuf> {
    let fixture = fixture(name);
    fixture
        .artifacts
        .get(profile)
        .map(|artifact| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&artifact.path))
}

pub fn get_fixture_path(name: &str) -> PathBuf {
    artifact_path(name, "release")
}

pub fn fixture_exists(name: &str) -> bool {
    get_fixture_path(name).exists()
}

pub fn artifact_exists(name: &str, profile: &str) -> bool {
    try_artifact_path(name, profile)
        .map(|path| path.exists())
        .unwrap_or(false)
}

pub fn load_fixture(name: &str) -> Vec<u8> {
    let path = get_fixture_path(name);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "Failed to read fixture '{}' from {}: {}",
            name,
            path.display(),
            e
        );
    })
}

pub fn source_path(name: &str) -> PathBuf {
    let fixture = fixture(name);
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(fixture.source.lib_rs)
}

pub fn contract_dir(name: &str) -> PathBuf {
    let fixture = fixture(name);
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(fixture.source.contract_dir)
}

pub fn relative_to_repo(path: &Path) -> String {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Available fixture contracts.
pub mod names {
    pub const COUNTER: &str = "counter";
    pub const ECHO: &str = "echo";
    pub const ALWAYS_PANIC: &str = "always_panic";
    pub const PANIC: &str = ALWAYS_PANIC;
    pub const BUDGET_HEAVY: &str = "budget_heavy";
    pub const CROSS_CONTRACT: &str = "cross_contract";
    pub const SAME_RETURN: &str = "same_return";
}
