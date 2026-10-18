//! Explicitly pinned local GridStrike packages for isolated native UI tests only.
//! This module is absent from default and production-style builds.

use crate::{
    InstallPlan, InstalledState, Release, current_target,
    installer::{InstallRoot, Phase},
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::Read,
    path::{Path, PathBuf},
};

const MAX_SPEC_BYTES: u64 = 32 * 1024;
const MARKER: &str = "PWDS UI fixture root v1\n";
const MARKER_NAME: &str = ".pwds-ui-fixture-root";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureSpec {
    schema: u32,
    releases: Vec<FixtureSpecRelease>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureSpecRelease {
    version: String,
    archive: PathBuf,
    sha256: String,
    bytes: u64,
}

struct FixtureRelease {
    release: Release,
    archive: PathBuf,
}

/// A deliberately separate source of local test packages. It never creates a
/// TrustedManifest or grants a local path to the normal remote download path.
pub struct FixtureSet {
    releases: Vec<FixtureRelease>,
}

impl FixtureSet {
    pub fn load(path: &Path) -> Result<Self, String> {
        let file = File::open(path).map_err(|e| format!("open fixture spec: {e}"))?;
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_SPEC_BYTES {
            return Err("fixture spec exceeds 32 KiB".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_SPEC_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| format!("read fixture spec: {e}"))?;
        if bytes.len() as u64 > MAX_SPEC_BYTES {
            return Err("fixture spec exceeds 32 KiB".into());
        }
        let spec: FixtureSpec =
            serde_json::from_slice(&bytes).map_err(|e| format!("invalid fixture spec: {e}"))?;
        if spec.schema != 1 || spec.releases.is_empty() || spec.releases.len() > 2 {
            return Err("fixture requires schema 1 and one or two GridStrike packages".into());
        }
        let (platform, arch) = current_target();
        let mut releases = Vec::with_capacity(spec.releases.len());
        let mut last_version = None;
        for item in spec.releases {
            if !matches!(item.version.as_str(), "0.5.0" | "0.5.1") {
                return Err("fixture version must be 0.5.0 or synthetic 0.5.1".into());
            }
            let key = crate::version_key(&item.version)?;
            if last_version.is_some_and(|last| key <= last) {
                return Err("fixture versions must increase without duplicates".into());
            }
            last_version = Some(key);
            let release = Release {
                app_id: "gridstrike".into(),
                version: item.version,
                platform: platform.into(),
                arch: arch.into(),
                artifact_url: String::new(),
                artifact_sha256: item.sha256,
                artifact_bytes: item.bytes,
                entrypoint: "gridstrike.exe".into(),
            };
            crate::validate_release_payload(&release)?;
            if !item.archive.is_absolute() {
                return Err("fixture archive path must be absolute".into());
            }
            let metadata = fs::symlink_metadata(&item.archive)
                .map_err(|e| format!("inspect fixture archive: {e}"))?;
            if !metadata.is_file() || linked(&metadata) {
                return Err("fixture archive must be a regular file".into());
            }
            if metadata.len() != release.artifact_bytes {
                return Err("fixture byte count mismatch".into());
            }
            let archive = item
                .archive
                .canonicalize()
                .map_err(|e| format!("canonicalize fixture archive: {e}"))?;
            let mut hash = Sha256::new();
            let mut source =
                File::open(&archive).map_err(|e| format!("open fixture archive: {e}"))?;
            let mut counted = 0u64;
            let mut buf = [0u8; 64 * 1024];
            loop {
                let count = source
                    .read(&mut buf)
                    .map_err(|e| format!("read fixture archive: {e}"))?;
                if count == 0 {
                    break;
                }
                counted = counted
                    .checked_add(count as u64)
                    .ok_or("fixture size overflow")?;
                if counted > release.artifact_bytes {
                    return Err("fixture byte count mismatch".into());
                }
                hash.update(&buf[..count]);
            }
            if counted != release.artifact_bytes {
                return Err("fixture byte count mismatch".into());
            }
            if format!("{:x}", hash.finalize()) != release.artifact_sha256.to_ascii_lowercase() {
                return Err("fixture SHA-256 mismatch".into());
            }
            releases.push(FixtureRelease { release, archive });
        }
        Ok(Self { releases })
    }

    pub fn first_version(&self) -> &str {
        &self.releases[0].release.version
    }

    pub fn plan<'a>(&'a self, state: &InstalledState, id: &str) -> Result<InstallPlan<'a>, String> {
        if id != "gridstrike" {
            return Err("only GridStrike has a local UI fixture".into());
        }
        let old = state.receipt(id);
        let old_key = old
            .map(|item| crate::version_key(&item.version))
            .transpose()?;
        let item = self
            .releases
            .iter()
            .find(|item| {
                old_key.is_none_or(|key| {
                    crate::version_key(&item.release.version).is_ok_and(|next| next > key)
                })
            })
            .ok_or("no newer GridStrike fixture")?;
        Ok(InstallPlan {
            release: &item.release,
            is_update: old.is_some(),
        })
    }

    pub fn install_next(
        &self,
        root: &InstallRoot,
        progress: &mut dyn FnMut(Phase) -> bool,
    ) -> Result<String, String> {
        let state = root.state()?;
        let plan = self.plan(&state, "gridstrike")?;
        let item = self
            .releases
            .iter()
            .find(|item| item.release.version == plan.release.version)
            .ok_or("planned fixture not found")?;
        root.install_archive(&plan, &item.archive, progress)?;
        Ok(plan.release.version.clone())
    }
}

pub fn check_isolated_root(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("fixture root must be absolute".into());
    }
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid fixture root name")?;
    let suffix = name
        .strip_prefix("pwds-ui-fixture-")
        .ok_or("fixture root needs pwds-ui-fixture- prefix")?;
    if suffix.is_empty()
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("invalid fixture root name".into());
    }
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|e| format!("canonicalize temp root: {e}"))?;
    let parent = path
        .parent()
        .ok_or("fixture root has no parent")?
        .canonicalize()
        .map_err(|e| format!("canonicalize fixture root parent: {e}"))?;
    if !parent.starts_with(&temp) {
        return Err("fixture root must be beneath the system temp directory".into());
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() || linked(&metadata) {
                return Err("fixture root is not a real directory".into());
            }
            let marker = path.join(MARKER_NAME);
            let marker_meta = fs::symlink_metadata(&marker)
                .map_err(|e| format!("inspect fixture root marker: {e}"))?;
            if !marker_meta.is_file() || linked(&marker_meta) {
                return Err("fixture root marker is not a real file".into());
            }
            if marker_meta.len() != MARKER.len() as u64 {
                return Err("fixture root marker mismatch".into());
            }
            if fs::read(marker).map_err(|e| format!("read fixture root marker: {e}"))?
                != MARKER.as_bytes()
            {
                return Err("fixture root marker mismatch".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("inspect fixture root: {error}")),
    }
    Ok(())
}

pub fn open_isolated_root(path: &Path) -> Result<InstallRoot, String> {
    check_isolated_root(path)?;
    if !path.exists() {
        fs::create_dir(path).map_err(|e| format!("create fixture root: {e}"))?;
        let marker = path.join(MARKER_NAME);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(marker)
            .map_err(|e| format!("create fixture root marker: {e}"))?;
        use std::io::Write;
        file.write_all(MARKER.as_bytes())
            .map_err(|e| format!("write fixture root marker: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("sync fixture root marker: {e}"))?;
    }
    InstallRoot::open(path)
}

fn linked(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
