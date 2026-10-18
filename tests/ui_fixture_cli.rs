#![cfg(all(feature = "native-ui", feature = "ui-local-fixture"))]

use sha2::{Digest, Sha256};
use std::{fs, io::Write, process::Command};
use tempfile::tempdir;

fn fixture_zip(path: &std::path::Path) -> (u64, String) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("gridstrike.exe", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"synthetic fixture executable").unwrap();
    zip.finish().unwrap();
    let data = fs::read(path).unwrap();
    (data.len() as u64, format!("{:x}", Sha256::digest(&data)))
}

#[test]
fn checked_ui_fixture_requires_exact_explicit_hash_without_creating_root() {
    let temp = tempdir().unwrap();
    let archive = temp.path().join("gridstrike.zip");
    let (bytes, hash) = fixture_zip(&archive);
    let spec = temp.path().join("fixture.json");
    let root = temp.path().join("pwds-ui-fixture-test");
    let mut release = serde_json::json!({
        "version": "0.5.0", "archive": archive, "sha256": hash, "bytes": bytes
    });
    fs::write(&spec, serde_json::to_vec(&serde_json::json!({
        "schema": 1, "releases": [release.clone()]
    })).unwrap()).unwrap();
    let valid = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args(["--check-local-fixture", spec.to_str().unwrap(), root.to_str().unwrap()])
        .output().unwrap();
    assert!(valid.status.success(), "{}", String::from_utf8_lossy(&valid.stderr));
    assert!(String::from_utf8_lossy(&valid.stdout).contains("GridStrike 0.5.0"));
    assert!(!root.exists(), "a check must not create install state");

    release["sha256"] = serde_json::json!("0".repeat(64));
    fs::write(&spec, serde_json::to_vec(&serde_json::json!({
        "schema": 1, "releases": [release]
    })).unwrap()).unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args(["--check-local-fixture", spec.to_str().unwrap(), root.to_str().unwrap()])
        .output().unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("fixture SHA-256 mismatch"));
    assert!(!root.exists());
}
