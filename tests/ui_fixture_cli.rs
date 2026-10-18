#![cfg(all(feature = "native-ui", feature = "ui-local-fixture"))]

use sha2::{Digest, Sha256};
use std::{fs, io::Write, process::Command};
use tempfile::tempdir;

fn fixture_zip(path: &std::path::Path, payload: &[u8]) -> (u64, String) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file("gridstrike.exe", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(payload).unwrap();
    zip.finish().unwrap();
    let data = fs::read(path).unwrap();
    (data.len() as u64, format!("{:x}", Sha256::digest(&data)))
}

fn fixture_zip_with_entry(path: &std::path::Path, entry: &str) -> (u64, String) {
    let file = fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    zip.start_file(entry, zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(b"synthetic fixture payload").unwrap();
    zip.finish().unwrap();
    let data = fs::read(path).unwrap();
    (data.len() as u64, format!("{:x}", Sha256::digest(&data)))
}

#[test]
fn checked_ui_fixture_requires_exact_explicit_hash_without_creating_root() {
    let temp = tempdir().unwrap();
    let archive = temp.path().join("gridstrike.zip");
    let (bytes, hash) = fixture_zip(&archive, b"synthetic fixture executable");
    let spec = temp.path().join("fixture.json");
    let root = temp.path().join("pwds-ui-fixture-test");
    let mut release = serde_json::json!({
        "version": "0.5.0", "archive": archive, "sha256": hash, "bytes": bytes
    });
    fs::write(
        &spec,
        serde_json::to_vec(&serde_json::json!({
            "schema": 1, "releases": [release.clone()]
        }))
        .unwrap(),
    )
    .unwrap();
    let valid = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args([
            "--check-local-fixture",
            spec.to_str().unwrap(),
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        valid.status.success(),
        "{}",
        String::from_utf8_lossy(&valid.stderr)
    );
    assert!(String::from_utf8_lossy(&valid.stdout).contains("GridStrike 0.5.0"));
    assert!(!root.exists(), "a check must not create install state");

    release["sha256"] = serde_json::json!("0".repeat(64));
    fs::write(
        &spec,
        serde_json::to_vec(&serde_json::json!({
            "schema": 1, "releases": [release]
        }))
        .unwrap(),
    )
    .unwrap();
    let invalid = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args([
            "--check-local-fixture",
            spec.to_str().unwrap(),
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("fixture SHA-256 mismatch"));
    assert!(!root.exists());
}

#[test]
fn local_ui_fixture_runs_install_update_and_rollback_in_isolated_root() {
    use pwds_strike_launcher::installer::InstallRoot;

    let temp = tempdir().unwrap();
    let root = temp.path().join("pwds-ui-fixture-transaction");
    let first = temp.path().join("first.zip");
    let second = temp.path().join("second.zip");
    let (first_bytes, first_hash) = fixture_zip(&first, b"gridstrike fixture version one");
    let (second_bytes, second_hash) = fixture_zip(&second, b"gridstrike fixture version two");
    let spec = temp.path().join("fixture.json");
    fs::write(&spec, serde_json::to_vec(&serde_json::json!({
        "schema": 1,
        "releases": [
            {"version": "0.5.0", "archive": first, "sha256": first_hash, "bytes": first_bytes},
            {"version": "0.5.1", "archive": second, "sha256": second_hash, "bytes": second_bytes}
        ]
    })).unwrap()).unwrap();

    for expected in ["0.5.0", "0.5.1"] {
        let output = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
            .args([
                "--local-fixture-install",
                spec.to_str().unwrap(),
                root.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected));
        assert_eq!(
            InstallRoot::open(&root)
                .unwrap()
                .state()
                .unwrap()
                .receipt("gridstrike")
                .unwrap()
                .version,
            expected
        );
    }
    assert!(InstallRoot::open(&root).unwrap().can_rollback("gridstrike"));
    InstallRoot::open(&root)
        .unwrap()
        .rollback("gridstrike")
        .unwrap();
    assert_eq!(
        InstallRoot::open(&root)
            .unwrap()
            .state()
            .unwrap()
            .receipt("gridstrike")
            .unwrap()
            .version,
        "0.5.0"
    );
}

#[test]
fn fixture_check_rejects_unmarked_or_outside_temp_root() {
    let temp = tempdir().unwrap();
    let archive = temp.path().join("gridstrike.zip");
    let (bytes, hash) = fixture_zip(&archive, b"synthetic fixture executable");
    let spec = temp.path().join("fixture.json");
    fs::write(&spec, serde_json::to_vec(&serde_json::json!({
        "schema": 1, "releases": [{"version":"0.5.0", "archive":archive, "sha256":hash, "bytes":bytes}]
    })).unwrap()).unwrap();
    let unmarked = temp.path().join("pwds-ui-fixture-unmarked");
    fs::create_dir(&unmarked).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args([
            "--check-local-fixture",
            spec.to_str().unwrap(),
            unmarked.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("fixture root marker"));
    assert!(fs::read_dir(&unmarked).unwrap().next().is_none());

    let outside = std::env::current_dir()
        .unwrap()
        .join("target/pwds-ui-fixture-outside");
    let result = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args([
            "--check-local-fixture",
            spec.to_str().unwrap(),
            outside.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("beneath the system temp"));
    assert!(!outside.exists());
}

#[test]
fn fixture_install_rejects_traversal_without_activating_receipt() {
    use pwds_strike_launcher::installer::InstallRoot;
    let temp = tempdir().unwrap();
    let archive = temp.path().join("traversal.zip");
    let (bytes, hash) = fixture_zip_with_entry(&archive, "../escape.txt");
    let spec = temp.path().join("fixture.json");
    let root = temp.path().join("pwds-ui-fixture-traversal");
    fs::write(&spec, serde_json::to_vec(&serde_json::json!({
        "schema": 1, "releases": [{"version":"0.5.0", "archive":archive, "sha256":hash, "bytes":bytes}]
    })).unwrap()).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_pwds-strike-launcher-ui"))
        .args([
            "--local-fixture-install",
            spec.to_str().unwrap(),
            root.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        InstallRoot::open(&root)
            .unwrap()
            .state()
            .unwrap()
            .receipt("gridstrike")
            .is_none()
    );
    assert!(!temp.path().join("escape.txt").exists());
}

#[test]
fn fixture_install_rechecks_archive_after_initial_pin_validation() {
    use pwds_strike_launcher::ui_fixture::{FixtureSet, open_isolated_root};
    let temp = tempdir().unwrap();
    let archive = temp.path().join("gridstrike.zip");
    let (bytes, hash) = fixture_zip(&archive, b"synthetic fixture executable");
    let spec = temp.path().join("fixture.json");
    let root_path = temp.path().join("pwds-ui-fixture-recheck");
    fs::write(&spec, serde_json::to_vec(&serde_json::json!({
        "schema": 1, "releases": [{"version":"0.5.0", "archive":archive, "sha256":hash, "bytes":bytes}]
    })).unwrap()).unwrap();
    let fixture = FixtureSet::load(&spec).unwrap();
    fs::write(&archive, b"changed after fixture validation").unwrap();
    let root = open_isolated_root(&root_path).unwrap();
    assert!(fixture.install_next(&root, &mut |_| true).is_err());
    assert!(root.state().unwrap().receipt("gridstrike").is_none());
}
