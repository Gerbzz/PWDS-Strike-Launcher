//! Per-user, launcher-owned ZIP installations. All paths derive from validated app IDs and versions.
use crate::{
    InstallPlan, InstalledFile, InstalledState, Receipt, app, current_target, validate_entrypoint,
    validate_release, validate_release_payload, verify_artifact,
};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::{Builder, NamedTempFile};
use url::Url;
use zip::ZipArchive;

const MAX_ENTRIES: usize = 20_000;
const MAX_UNPACKED: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Downloading,
    DownloadBytes(u64, u64),
    Verifying,
    Extracting,
    Activating,
    Complete,
}

pub struct InstallRoot {
    path: PathBuf,
}

impl InstallRoot {
    pub fn open(path: &Path) -> Result<Self, String> {
        fs::create_dir_all(path).map_err(|e| format!("create install root: {e}"))?;
        guard_dir(path)?;
        let path = path
            .canonicalize()
            .map_err(|e| format!("canonicalize install root: {e}"))?;
        let root = Self { path };
        root.recover()?;
        Ok(root)
    }

    fn state_path(&self) -> PathBuf {
        self.path.join("installed.json")
    }
    fn backup_path(&self) -> PathBuf {
        self.path.join("installed.backup.json")
    }
    fn operation_lock(&self) -> Result<File, String> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(self.path.join("operation.lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|_| "another launcher operation is in progress".to_string())?;
        Ok(file)
    }
    fn app_path(&self, id: &str) -> Result<PathBuf, String> {
        if app(id).is_none() {
            return Err("unknown app ID".into());
        }
        let path = self.path.join("apps").join(id);
        guard_existing_ancestors(&self.path, &path)?;
        Ok(path)
    }
    fn version_path(&self, receipt: &Receipt) -> Result<PathBuf, String> {
        crate::version_key(&receipt.version)?;
        let path = self.app_path(&receipt.app_id)?.join(&receipt.version);
        guard_existing_ancestors(&self.path, &path)?;
        Ok(path)
    }

    /// An interrupted state-file swap restores the last complete receipt file.
    pub fn recover(&self) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        let state = self.state_path();
        let backup = self.backup_path();
        if !state.exists() && backup.exists() {
            guard_file(&backup)?;
            InstalledState::load(&backup)?;
            fs::rename(&backup, &state).map_err(|e| format!("restore installed state: {e}"))?;
        }
        self.state().map(|_| ())
    }

    pub fn state(&self) -> Result<InstalledState, String> {
        InstalledState::load(&self.state_path())
    }

    fn commit(&self, new_state: &InstalledState) -> Result<(), String> {
        new_state.validate()?;
        let mut temp = NamedTempFile::new_in(&self.path).map_err(|e| e.to_string())?;
        serde_json::to_writer(&mut temp, new_state).map_err(|e| e.to_string())?;
        temp.as_file_mut().sync_all().map_err(|e| e.to_string())?;
        let temp_path = temp.into_temp_path();
        let state = self.state_path();
        let backup = self.backup_path();
        if backup.exists() {
            guard_file(&backup)?;
            fs::remove_file(&backup).map_err(|e| e.to_string())?;
        }
        let had_state = state.exists();
        if had_state {
            guard_file(&state)?;
            fs::rename(&state, &backup).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&temp_path, &state) {
            if had_state {
                let _ = fs::rename(&backup, &state);
            }
            return Err(format!("commit installed state: {error}"));
        }
        OpenOptions::new()
            .write(true)
            .open(&state)
            .and_then(|file| file.sync_all())
            .map_err(|e| format!("sync installed state: {e}"))?;
        Ok(())
    }

    pub fn install_archive(
        &self,
        plan: &InstallPlan<'_>,
        archive: &Path,
        progress: &mut dyn FnMut(Phase) -> bool,
    ) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        let release = plan.release;
        validate_release_payload(release)?;
        if (release.platform.as_str(), release.arch.as_str()) != current_target() {
            return Err("release is for another platform".into());
        }
        let mut state = self.state()?;
        let old = state.receipt(&release.app_id);
        if old.is_some() != plan.is_update {
            return Err("installation state changed; plan again".into());
        }
        if let Some(old) = old {
            if crate::version_key(&release.version)? <= crate::version_key(&old.version)? {
                return Err("release is not newer than installed version".into());
            }
            self.verify_receipt(old)?;
        }
        if !progress(Phase::Verifying) {
            return Err("installation cancelled".into());
        }
        verify_artifact(archive, release)?;
        let app_dir = self.app_path(&release.app_id)?;
        fs::create_dir_all(&app_dir).map_err(|e| e.to_string())?;
        guard_dir(&app_dir)?;
        let target = app_dir.join(&release.version);
        if target.exists() {
            return Err("version directory already exists".into());
        }
        if !progress(Phase::Extracting) {
            return Err("installation cancelled".into());
        }
        let stage = Builder::new()
            .prefix(".stage-")
            .tempdir_in(&app_dir)
            .map_err(|e| e.to_string())?;
        let files = extract_archive(archive, stage.path(), progress)?;
        if !files.iter().any(|f| f.path == release.entrypoint) {
            return Err("entrypoint absent from archive".into());
        }
        let staged_entry = stage.path().join(&release.entrypoint);
        guard_file(&staged_entry)?;
        if !progress(Phase::Activating) {
            return Err("installation cancelled".into());
        }
        let receipt = Receipt {
            app_id: release.app_id.clone(),
            version: release.version.clone(),
            platform: release.platform.clone(),
            arch: release.arch.clone(),
            entrypoint: release.entrypoint.clone(),
            files,
        };
        let staged_path = stage.keep();
        fs::rename(&staged_path, &target)
            .map_err(|e| format!("activate version directory: {e}"))?;
        if let Some(old) = state.receipt(&release.app_id).cloned() {
            state.retained.push(old);
        }
        state.installed.retain(|r| r.app_id != release.app_id);
        state.installed.push(receipt);
        if let Err(error) = self.commit(&state) {
            // No receipt points to the new version; keep the directory for inspection.
            return Err(format!(
                "{error}; staged version remains at {}",
                target.display()
            ));
        }
        progress(Phase::Complete);
        Ok(())
    }

    /// Fetches only a release URL selected from the embedded manifest. GitHub's asset redirect
    /// is checked explicitly, and the exact pinned bytes are verified before ZIP parsing.
    pub fn download_and_install(
        &self,
        plan: &InstallPlan<'_>,
        progress: &mut dyn FnMut(Phase) -> bool,
    ) -> Result<(), String> {
        let release = plan.release;
        validate_release(release)?;
        if !progress(Phase::Downloading) {
            return Err("installation cancelled".into());
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(120)))
            .timeout_connect(Some(std::time::Duration::from_secs(15)))
            .build()
            .into();
        let mut url = release.artifact_url.clone();
        let mut response = None;
        for hop in 0..=2 {
            let res = agent
                .get(&url)
                .call()
                .map_err(|e| format!("download failed: {e}"))?;
            let status = res.status().as_u16();
            if status == 200 {
                response = Some(res);
                break;
            }
            if !matches!(status, 301 | 302 | 303 | 307 | 308) || hop == 2 {
                return Err(format!("unexpected download response: {status}"));
            }
            let location = res
                .headers()
                .get("location")
                .and_then(|h| h.to_str().ok())
                .ok_or("release redirect lacks Location")?;
            let next = Url::parse(location).map_err(|_| "invalid release redirect")?;
            if next.scheme() != "https"
                || next.port().is_some()
                || !next.username().is_empty()
                || next.password().is_some()
                || !matches!(
                    next.host_str(),
                    Some(
                        "release-assets.githubusercontent.com"
                            | "objects.githubusercontent.com"
                            | "github-releases.githubusercontent.com"
                    )
                )
            {
                return Err("release redirect leaves approved GitHub asset hosts".into());
            }
            url = next.to_string();
        }
        let mut response = response.ok_or("release download did not return an artifact")?;
        if let Some(size) = response
            .headers()
            .get("content-length")
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
        {
            if size > release.artifact_bytes {
                return Err("download exceeds pinned byte count".into());
            }
        }
        let mut temp =
            NamedTempFile::new_in(&self.path).map_err(|e| format!("create download stage: {e}"))?;
        let mut reader = response.body_mut().as_reader();
        let mut bytes = 0u64;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("read download: {e}"))?;
            if n == 0 {
                break;
            }
            bytes = bytes
                .checked_add(n as u64)
                .ok_or("download size overflow")?;
            if bytes > release.artifact_bytes {
                return Err("download exceeds pinned byte count".into());
            }
            if !progress(Phase::DownloadBytes(bytes, release.artifact_bytes)) {
                return Err("installation cancelled".into());
            }
            temp.write_all(&buf[..n])
                .map_err(|e| format!("write download: {e}"))?;
        }
        if bytes != release.artifact_bytes {
            return Err("download is shorter than pinned byte count".into());
        }
        temp.as_file_mut().sync_all().map_err(|e| e.to_string())?;
        let path = temp.into_temp_path();
        self.install_archive(plan, &path, progress)
    }

    pub fn entrypoint(&self, id: &str) -> Result<PathBuf, String> {
        let state = self.state()?;
        let receipt = state.receipt(id).ok_or("app is not installed")?;
        self.verify_receipt(receipt)?;
        Ok(self.version_path(receipt)?.join(&receipt.entrypoint))
    }

    pub fn launch(&self, id: &str, args: &[String]) -> Result<u32, String> {
        let entry = self.entrypoint(id)?;
        let working_dir = entry.parent().ok_or("invalid entrypoint")?;
        match Command::new(&entry)
            .args(args)
            .current_dir(working_dir)
            .spawn()
        {
            Ok(child) => Ok(child.id()),
            Err(error) => {
                // A failed first launch can be reversed when a previous receipt is available.
                if self.backup_path().exists() {
                    let _ = self.rollback(id);
                }
                Err(format!("launch failed: {error}"))
            }
        }
    }

    pub fn rollback(&self, id: &str) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        let mut old = InstalledState::load(&self.backup_path())?;
        let receipt = old.receipt(id).ok_or("no previous installed version")?;
        self.verify_receipt(receipt)?;
        if let Some(current) = self.state()?.receipt(id).cloned() {
            old.retained
                .retain(|r| !(r.app_id == id && r.version == current.version));
            if old
                .receipt(id)
                .is_some_and(|r| r.version != current.version)
            {
                old.retained.push(current);
            }
        }
        self.commit(&old)
    }

    pub fn can_rollback(&self, id: &str) -> bool {
        InstalledState::load(&self.backup_path())
            .ok()
            .and_then(|s| s.receipt(id).cloned())
            .is_some_and(|r| self.verify_receipt(&r).is_ok())
    }

    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        let mut state = self.state()?;
        let receipt = state.receipt(id).ok_or("app is not installed")?.clone();
        self.verify_receipt(&receipt)?;
        let mut owned = vec![receipt.clone()];
        for retained in state.retained.iter().filter(|r| r.app_id == id) {
            if !owned.iter().any(|r| r.version == retained.version) {
                self.verify_receipt(retained)?;
                owned.push(retained.clone());
            }
        }
        let previous = InstalledState::load(&self.backup_path())?;
        if let Some(old) = previous.receipt(id) {
            if !owned.iter().any(|r| r.version == old.version) {
                self.verify_receipt(old)?;
                owned.push(old.clone());
            }
        }
        let app_dir = self.app_path(id)?;
        let mut parked_versions = Vec::new();
        for owned_receipt in &owned {
            let target = self.version_path(owned_receipt)?;
            let parked = Builder::new()
                .prefix(".uninstall-")
                .tempdir_in(&app_dir)
                .map_err(|e| e.to_string())?;
            let parked_path = parked.keep();
            fs::remove_dir(&parked_path).map_err(|e| e.to_string())?;
            if let Err(error) = fs::rename(&target, &parked_path) {
                for (target, parked) in parked_versions.iter().rev() {
                    let _ = fs::rename(parked, target);
                }
                return Err(format!("park installed files: {error}"));
            }
            parked_versions.push((target, parked_path));
        }
        state.installed.retain(|r| r.app_id != id);
        state.retained.retain(|r| r.app_id != id);
        if let Err(error) = self.commit(&state) {
            for (target, parked) in parked_versions.iter().rev() {
                let _ = fs::rename(parked, target);
            }
            return Err(error);
        }
        // Only fully inventoried, launcher-owned version directories are removed.
        for (_, parked) in parked_versions {
            fs::remove_dir_all(&parked).map_err(|e| {
                format!("uninstalled; cleanup remains at {}: {e}", parked.display())
            })?;
        }
        Ok(())
    }

    fn verify_receipt(&self, receipt: &Receipt) -> Result<(), String> {
        if receipt.files.is_empty() {
            return Err("receipt has no verified file inventory".into());
        }
        let root = self.version_path(receipt)?;
        guard_dir(&root)?;
        let expected: HashSet<_> = receipt
            .files
            .iter()
            .map(|f| f.path.to_ascii_lowercase())
            .collect();
        let mut actual = HashSet::new();
        scan_files(&root, &root, &mut actual)?;
        if expected != actual {
            return Err("installed file inventory changed".into());
        }
        for item in &receipt.files {
            let file = root.join(&item.path);
            guard_file(&file)?;
            let (bytes, sha) = file_digest(&file)?;
            if bytes != item.bytes || sha != item.sha256 {
                return Err("installed file hash changed".into());
            }
        }
        if !expected.contains(&receipt.entrypoint.to_ascii_lowercase()) {
            return Err("entrypoint missing from inventory".into());
        }
        Ok(())
    }
}

fn extract_archive(
    archive: &Path,
    root: &Path,
    progress: &mut dyn FnMut(Phase) -> bool,
) -> Result<Vec<InstalledFile>, String> {
    let mut zip = ZipArchive::new(File::open(archive).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if zip.len() == 0 || zip.len() > MAX_ENTRIES {
        return Err("archive entry count outside limits".into());
    }
    let mut names = HashSet::new();
    let mut total = 0u64;
    for index in 0..zip.len() {
        let entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().trim_end_matches('/');
        validate_entrypoint(name)?;
        let folded = name.to_ascii_lowercase();
        if !names.insert(folded) {
            return Err("duplicate archive path".into());
        }
        let mode = entry.unix_mode().unwrap_or(0) & 0o170000;
        if mode != 0 && mode != if entry.is_dir() { 0o040000 } else { 0o100000 } {
            return Err("archive contains a link or special file".into());
        }
        if !entry.is_dir() {
            total = total
                .checked_add(entry.size())
                .ok_or("archive size overflow")?;
            if total > MAX_UNPACKED
                || entry.size() > entry.compressed_size().saturating_mul(200).max(1024 * 1024)
            {
                return Err("archive expands beyond supported limits".into());
            }
        }
    }
    let mut files = Vec::new();
    for index in 0..zip.len() {
        if !progress(Phase::Extracting) {
            return Err("installation cancelled".into());
        }
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().trim_end_matches('/').to_string();
        let destination = root.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            guard_dir(&destination)?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            guard_existing_ancestors(root, parent)?;
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(|e| e.to_string())?;
        let mut digest = Sha256::new();
        let mut bytes = 0u64;
        let mut buf = [0u8; 64 * 1024];
        loop {
            if !progress(Phase::Extracting) {
                return Err("installation cancelled".into());
            }
            let n = entry.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            bytes += n as u64;
            if bytes > entry.size() || bytes > MAX_UNPACKED {
                return Err("archive entry expanded beyond limit".into());
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            digest.update(&buf[..n]);
        }
        if bytes != entry.size() {
            return Err("archive entry size mismatch".into());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        files.push(InstalledFile {
            path: name,
            bytes,
            sha256: format!("{:x}", digest.finalize()),
        });
    }
    Ok(files)
}

fn file_digest(path: &Path) -> Result<(u64, String), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count += n as u64;
        digest.update(&buf[..n]);
    }
    Ok((count, format!("{:x}", digest.finalize())))
}

fn scan_files(root: &Path, path: &Path, found: &mut HashSet<String>) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if is_link(&metadata) {
            return Err("installed directory contains a link".into());
        }
        if metadata.is_dir() {
            scan_files(root, &path, found)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            found.insert(relative.to_ascii_lowercase());
        } else {
            return Err("installed directory contains a special file".into());
        }
    }
    Ok(())
}

fn guard_existing_ancestors(root: &Path, path: &Path) -> Result<(), String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| "path escapes install root")?;
    let mut cursor = root.to_path_buf();
    for part in rel.components() {
        cursor.push(part);
        if cursor.exists() {
            guard_dir(&cursor)?;
        }
    }
    Ok(())
}
fn guard_dir(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_dir() || is_link(&meta) {
        return Err("install directory is a link or special file".into());
    }
    Ok(())
}
fn guard_file(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !meta.is_file() || is_link(&meta) {
        return Err("installed file is a link or special file".into());
    }
    Ok(())
}
fn is_link(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if meta.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}
