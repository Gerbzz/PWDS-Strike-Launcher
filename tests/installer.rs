use pwds_strike_launcher::{
    InstallPlan, Release,
    installer::{InstallRoot, Phase},
};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

fn package(path: &Path, files: &[(&str, &[u8])]) -> Release {
    let file = fs::File::create(path).unwrap();
    let mut zip = ZipWriter::new(file);
    for (name, bytes) in files {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap();
    let bytes = fs::read(path).unwrap();
    Release {
        app_id: "gridstrike".into(),
        version: "1.0.0".into(),
        platform: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        artifact_url: String::new(), // local synthetic package; never a download authority
        artifact_sha256: format!("{:x}", Sha256::digest(&bytes)),
        artifact_bytes: bytes.len() as u64,
        entrypoint: "gridstrike.exe".into(),
    }
}

fn root(tmp: &TempDir) -> InstallRoot {
    InstallRoot::open(&tmp.path().join("launcher")).unwrap()
}

fn fixture() -> TempDir {
    TempDir::new_in(std::env::current_dir().unwrap()).unwrap()
}

#[test]
fn temp_directory_can_be_renamed_on_this_platform() {
    let tmp = fixture();
    let plain = tmp.path().join("plain");
    fs::create_dir(&plain).unwrap();
    fs::rename(&plain, tmp.path().join("plain2")).unwrap();
    let stage = tempfile::Builder::new()
        .prefix(".stage-")
        .tempdir_in(tmp.path())
        .unwrap();
    let old = stage.keep();
    fs::rename(&old, tmp.path().join("active")).unwrap();
}

#[test]
fn installs_verified_archive_and_preserves_user_data_on_uninstall() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let release = package(
        &archive,
        &[
            ("gridstrike.exe", b"binary"),
            ("resources/icon.png", b"icon"),
        ],
    );
    let root = root(&tmp);
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(
        root.state().unwrap().receipt("gridstrike").unwrap().version,
        "1.0.0"
    );
    assert_eq!(
        fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
        b"binary"
    );
    let data = tmp.path().join("user-projects.txt");
    fs::write(&data, b"keep").unwrap();
    root.uninstall("gridstrike").unwrap();
    assert!(root.state().unwrap().receipt("gridstrike").is_none());
    assert_eq!(fs::read(data).unwrap(), b"keep");
}

#[test]
fn corrupt_package_and_traversal_leave_existing_install_active() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let mut release = package(&archive, &[("gridstrike.exe", b"old")]);
    let root = root(&tmp);
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    release.version = "2.0.0".into();
    fs::write(&archive, b"broken").unwrap();
    assert!(
        root.install_archive(
            &InstallPlan {
                release: &release,
                is_update: true
            },
            &archive,
            &mut |_| true
        )
        .is_err()
    );
    assert_eq!(
        fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
        b"old"
    );

    let bad = tmp.path().join("traversal.zip");
    let mut release = package(
        &bad,
        &[("../outside.txt", b"escape"), ("gridstrike.exe", b"new")],
    );
    release.version = "2.0.0".into();
    assert!(
        root.install_archive(
            &InstallPlan {
                release: &release,
                is_update: true
            },
            &bad,
            &mut |_| true
        )
        .is_err()
    );
    assert!(!tmp.path().join("outside.txt").exists());
    assert_eq!(
        fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
        b"old"
    );
}

#[test]
fn cancellation_and_existing_version_do_not_change_receipt() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let release = package(&archive, &[("gridstrike.exe", b"old")]);
    let root = root(&tmp);
    let result = root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |phase| phase != Phase::Extracting,
    );
    assert!(result.is_err());
    assert!(root.state().unwrap().installed.is_empty());
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    assert!(
        root.install_archive(
            &InstallPlan {
                release: &release,
                is_update: false
            },
            &archive,
            &mut |_| true
        )
        .is_err()
    );
    assert_eq!(
        fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
        b"old"
    );
}

#[test]
fn update_can_roll_back_to_previous_verified_version() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let mut release = package(&archive, &[("gridstrike.exe", b"old")]);
    let root = root(&tmp);
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    release = package(&archive, &[("gridstrike.exe", b"new")]);
    release.version = "2.0.0".into();
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: true,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(
        fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
        b"new"
    );
    root.rollback("gridstrike").unwrap();
    assert_eq!(
        fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
        b"old"
    );
}

#[test]
fn interrupted_receipt_swap_recovers_last_complete_install() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let release = package(&archive, &[("gridstrike.exe", b"old")]);
    let root = root(&tmp);
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    let state = tmp.path().join("launcher/installed.json");
    let backup = tmp.path().join("launcher/installed.backup.json");
    fs::rename(&state, &backup).unwrap();
    let recovered = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
    assert_eq!(
        fs::read(recovered.entrypoint("gridstrike").unwrap()).unwrap(),
        b"old"
    );
}

#[test]
fn uninstall_refuses_unknown_files_and_keeps_receipt() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let release = package(&archive, &[("gridstrike.exe", b"old")]);
    let root = root(&tmp);
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    let user_file = tmp
        .path()
        .join("launcher/apps/gridstrike/1.0.0/my-project.txt");
    fs::write(&user_file, b"keep").unwrap();
    assert!(root.uninstall("gridstrike").is_err());
    assert_eq!(fs::read(&user_file).unwrap(), b"keep");
    assert!(root.state().unwrap().receipt("gridstrike").is_some());
}

#[test]
fn uninstall_removes_current_and_previous_owned_versions() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let mut release = package(&archive, &[("gridstrike.exe", b"old")]);
    let root = root(&tmp);
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: false,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    release = package(&archive, &[("gridstrike.exe", b"new")]);
    release.version = "2.0.0".into();
    root.install_archive(
        &InstallPlan {
            release: &release,
            is_update: true,
        },
        &archive,
        &mut |_| true,
    )
    .unwrap();
    root.uninstall("gridstrike").unwrap();
    let base = tmp.path().join("launcher/apps/gridstrike");
    assert!(!base.join("1.0.0").exists());
    assert!(!base.join("2.0.0").exists());
}

#[test]
fn uninstall_after_two_updates_removes_all_owned_versions() {
    let tmp = fixture();
    let archive = tmp.path().join("app.zip");
    let root = root(&tmp);
    for (version, bytes) in [
        ("1.0.0", b"one".as_slice()),
        ("2.0.0", b"two".as_slice()),
        ("3.0.0", b"three".as_slice()),
    ] {
        let mut release = package(&archive, &[("gridstrike.exe", bytes)]);
        release.version = version.into();
        root.install_archive(
            &InstallPlan {
                release: &release,
                is_update: version != "1.0.0",
            },
            &archive,
            &mut |_| true,
        )
        .unwrap();
    }
    root.uninstall("gridstrike").unwrap();
    let base = tmp.path().join("launcher/apps/gridstrike");
    for version in ["1.0.0", "2.0.0", "3.0.0"] {
        assert!(!base.join(version).exists(), "{version}");
    }
}
