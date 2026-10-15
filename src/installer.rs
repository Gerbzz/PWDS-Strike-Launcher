//! Per-user, launcher-owned ZIP installations. All paths derive from validated app IDs and versions.
use crate::{
    InstallPlan, InstalledFile, InstalledState, Receipt, app, current_target, validate_entrypoint,
    validate_release, validate_release_payload, verify_artifact,
};
use serde::{Deserialize, Serialize};
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
const MAX_UNINSTALL_JOURNAL_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Clone, Deserialize, Serialize)]
struct ParkedVersion {
    receipt: Receipt,
    parked_name: String,
}

#[derive(Deserialize, Serialize)]
struct UninstallJournal {
    schema: u32,
    app_id: String,
    versions: Vec<ParkedVersion>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Downloading,
    DownloadBytes(u64, u64),
    Verifying,
    Extracting,
    ExtractedEntries(usize, usize),
    Activating,
    Complete,
}

pub struct InstallRoot {
    path: PathBuf,
}

impl InstallRoot {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut ancestor = PathBuf::new();
        for component in path.components() {
            ancestor.push(component.as_os_str());
            match fs::symlink_metadata(&ancestor) {
                Ok(meta) if is_link(&meta) => {
                    return Err("install root contains a link or reparse point".into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(format!("inspect install root: {error}")),
            }
        }
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
    fn uninstall_journal_path(&self) -> PathBuf {
        self.path.join("uninstall-journal.json")
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
        self.recover_locked()
    }

    /// Every operation that can change installed state calls this while holding the root lock.
    /// A reused UI root must observe the same pending journal as a newly opened root.
    fn recover_locked(&self) -> Result<(), String> {
        let state = self.state_path();
        let backup = self.backup_path();
        if !state.exists() && backup.exists() {
            guard_file(&backup)?;
            InstalledState::load(&backup)?;
            fs::rename(&backup, &state).map_err(|e| format!("restore installed state: {e}"))?;
        }
        self.state()?;
        self.recover_uninstall()
    }

    fn validate_uninstall_journal(&self, journal: &UninstallJournal) -> Result<(), String> {
        if journal.schema != 1 || app(&journal.app_id).is_none() || journal.versions.is_empty() {
            return Err("invalid uninstall journal".into());
        }
        let mut versions = HashSet::new();
        let mut names = HashSet::new();
        for item in &journal.versions {
            if item.receipt.app_id != journal.app_id
                || !versions.insert(item.receipt.version.as_str())
                || !item.parked_name.starts_with(".uninstall-")
                || !names.insert(item.parked_name.as_str())
            {
                return Err("invalid uninstall journal entry".into());
            }
            validate_entrypoint(&item.parked_name)?;
            if item.parked_name.contains('/') {
                return Err("invalid uninstall parking name".into());
            }
            self.version_path(&item.receipt)?;
        }
        Ok(())
    }

    fn write_uninstall_journal(&self, journal: &UninstallJournal) -> Result<(), String> {
        self.validate_uninstall_journal(journal)?;
        let path = self.uninstall_journal_path();
        if path_occupied(&path)? {
            return Err("unfinished uninstall requires recovery".into());
        }
        let bytes = serde_json::to_vec(journal).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_UNINSTALL_JOURNAL_BYTES {
            return Err("uninstall journal exceeds 2 MiB".into());
        }
        let mut temp = NamedTempFile::new_in(&self.path).map_err(|e| e.to_string())?;
        temp.write_all(&bytes).map_err(|e| e.to_string())?;
        temp.as_file_mut().sync_all().map_err(|e| e.to_string())?;
        fs::rename(temp.into_temp_path(), &path)
            .map_err(|e| format!("commit uninstall journal: {e}"))
    }

    fn read_uninstall_journal(&self) -> Result<Option<UninstallJournal>, String> {
        let path = self.uninstall_journal_path();
        if !path_occupied(&path)? {
            return Ok(None);
        }
        guard_file(&path)?;
        if fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_UNINSTALL_JOURNAL_BYTES {
            return Err("uninstall journal exceeds 2 MiB".into());
        }
        let bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let journal: UninstallJournal = serde_json::from_slice(&bytes)
            .map_err(|e| format!("invalid uninstall journal: {e}"))?;
        self.validate_uninstall_journal(&journal)?;
        Ok(Some(journal))
    }

    fn recover_uninstall(&self) -> Result<(), String> {
        let Some(journal) = self.read_uninstall_journal()? else {
            return Ok(());
        };
        let still_installed = self.state()?.receipt(&journal.app_id).is_some();
        let app_dir = self.app_path(&journal.app_id)?;
        for item in journal.versions.iter().rev() {
            let target = self.version_path(&item.receipt)?;
            let parked = app_dir.join(&item.parked_name);
            let target_exists = path_occupied(&target)?;
            let parked_exists = path_occupied(&parked)?;
            if still_installed {
                match (target_exists, parked_exists) {
                    (false, true) => {
                        guard_dir(&parked)?;
                        fs::rename(&parked, &target)
                            .map_err(|e| format!("restore parked app version: {e}"))?;
                    }
                    (true, false) => {}
                    _ => return Err("uninstall recovery found conflicting version paths".into()),
                }
                self.verify_receipt(&item.receipt)?;
            } else if parked_exists {
                if target_exists {
                    return Err("uninstall cleanup found conflicting version paths".into());
                }
                // remove_dir_all can stop after deleting some owned files. In that case
                // only remaining verified files may be removed on the next opening.
                self.verify_remaining_inventory(&parked, &item.receipt)?;
                fs::remove_dir_all(&parked)
                    .map_err(|e| format!("complete interrupted uninstall: {e}"))?;
            } else if target_exists {
                return Err("uninstall cleanup found an unowned version directory".into());
            }
        }
        fs::remove_file(self.uninstall_journal_path())
            .map_err(|e| format!("finish uninstall recovery: {e}"))
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
        }
        if state.exists() {
            guard_file(&state)?;
            let mut source = File::open(&state).map_err(|e| e.to_string())?;
            let mut backup_temp = NamedTempFile::new_in(&self.path).map_err(|e| e.to_string())?;
            std::io::copy(&mut source, &mut backup_temp).map_err(|e| e.to_string())?;
            backup_temp
                .as_file_mut()
                .sync_all()
                .map_err(|e| e.to_string())?;
            let backup_temp_path = backup_temp.into_temp_path();
            fs::rename(&backup_temp_path, &backup)
                .map_err(|e| format!("save rollback receipt: {e}"))?;
        }
        // Same-volume replacement changes the visible active receipt in one rename.
        fs::rename(&temp_path, &state).map_err(|e| format!("commit installed state: {e}"))?;
        Ok(())
    }

    pub fn install_archive(
        &self,
        plan: &InstallPlan<'_>,
        archive: &Path,
        progress: &mut dyn FnMut(Phase) -> bool,
    ) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        self.recover_locked()?;
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
        let receipt = Receipt {
            app_id: release.app_id.clone(),
            version: release.version.clone(),
            platform: release.platform.clone(),
            arch: release.arch.clone(),
            entrypoint: release.entrypoint.clone(),
            files,
        };
        self.verify_inventory(stage.path(), &receipt)?;
        state
            .retained
            .retain(|old| !(old.app_id == release.app_id && old.version == release.version));
        if let Some(old) = state.receipt(&release.app_id).cloned() {
            state.retained.push(old);
        }
        state.installed.retain(|r| r.app_id != release.app_id);
        state.installed.push(receipt.clone());
        // The state loader enforces the same 1 MiB limit. Check the complete new receipt before
        // a version directory can become visible, including when reusing an interrupted stage.
        state.validate()?;
        if !progress(Phase::Activating) {
            return Err("installation cancelled".into());
        }
        // A process may have stopped after the version-directory rename but before committing
        // its receipt. Never delete that directory: adopt it on retry only when every file
        // matches the newly verified archive. The same check permits reactivating a retained
        // version after a user-initiated rollback.
        match fs::symlink_metadata(&target) {
            Ok(_) => self.verify_receipt(&receipt)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let staged_path = stage.keep();
                fs::rename(&staged_path, &target)
                    .map_err(|e| format!("activate version directory: {e}"))?;
            }
            Err(error) => return Err(format!("inspect version directory: {error}")),
        }
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
        self.recover()?;
        if !progress(Phase::Downloading) {
            return Err("installation cancelled".into());
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .timeout_global(Some(std::time::Duration::from_secs(120)))
            .timeout_connect(Some(std::time::Duration::from_secs(15)))
            .build()
            .into();
        let source = Url::parse(&release.artifact_url).map_err(|_| "invalid pinned release URL")?;
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
            if !approved_redirect(&source, &next) {
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
        let _lock = self.operation_lock()?;
        self.recover_locked()?;
        self.verified_entrypoint(id)
    }

    fn verified_entrypoint(&self, id: &str) -> Result<PathBuf, String> {
        let state = self.state()?;
        let receipt = state.receipt(id).ok_or("app is not installed")?;
        self.verify_receipt(receipt)?;
        Ok(self.version_path(receipt)?.join(&receipt.entrypoint))
    }

    pub fn launch(&self, id: &str, args: &[String]) -> Result<u32, String> {
        let lock = self.operation_lock()?;
        self.recover_locked()?;
        let entry = self.verified_entrypoint(id)?;
        let working_dir = entry.parent().ok_or("invalid entrypoint")?;
        let result = Command::new(&entry)
            .args(args)
            .current_dir(working_dir)
            .spawn();
        drop(lock);
        match result {
            Ok(child) => Ok(child.id()),
            Err(error) => {
                // A failed first launch can be reversed when a previous receipt is available.
                if self.can_rollback(id) {
                    let _ = self.rollback(id);
                }
                Err(format!("launch failed: {error}"))
            }
        }
    }

    pub fn rollback(&self, id: &str) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        self.recover_locked()?;
        let mut state = self.state()?;
        let current = state.receipt(id).cloned().ok_or("app is not installed")?;
        let previous_index = state
            .retained
            .iter()
            .rposition(|r| r.app_id == id && r.version != current.version)
            .ok_or("no distinct previous version")?;
        self.verify_receipt(&state.retained[previous_index])?;
        let previous = state.retained.remove(previous_index);
        state.retained.push(current);
        state.installed.retain(|r| r.app_id != id);
        state.installed.push(previous);
        self.commit(&state)
    }

    pub fn can_rollback(&self, id: &str) -> bool {
        self.state().ok().is_some_and(|state| {
            state.receipt(id).is_some_and(|current| {
                state
                    .retained
                    .iter()
                    .rev()
                    .find(|r| r.app_id == id && r.version != current.version)
                    .is_some_and(|previous| self.verify_receipt(previous).is_ok())
            })
        })
    }

    pub fn uninstall(&self, id: &str) -> Result<(), String> {
        self.uninstall_impl(id, &mut |_| Ok(()), &mut || Ok(()))
    }

    fn uninstall_impl(
        &self,
        id: &str,
        after_park: &mut dyn FnMut(usize) -> Result<(), String>,
        before_cleanup: &mut dyn FnMut() -> Result<(), String>,
    ) -> Result<(), String> {
        let _lock = self.operation_lock()?;
        self.recover_locked()?;
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
        let mut journal = UninstallJournal {
            schema: 1,
            app_id: id.to_string(),
            versions: Vec::new(),
        };
        // Reserve every parking name before moving the first installed version. A later
        // allocation failure therefore cannot strand an already parked version.
        for owned_receipt in &owned {
            let parked = Builder::new()
                .prefix(".uninstall-")
                .tempdir_in(&app_dir)
                .map_err(|e| e.to_string())?;
            let parked_path = parked.keep();
            fs::remove_dir(&parked_path).map_err(|e| e.to_string())?;
            journal.versions.push(ParkedVersion {
                receipt: owned_receipt.clone(),
                parked_name: parked_path
                    .file_name()
                    .ok_or("invalid uninstall parking path")?
                    .to_string_lossy()
                    .into_owned(),
            });
        }
        self.write_uninstall_journal(&journal)?;
        let operation = (|| {
            for (index, item) in journal.versions.iter().enumerate() {
                let target = self.version_path(&item.receipt)?;
                let parked = app_dir.join(&item.parked_name);
                fs::rename(&target, &parked).map_err(|e| format!("park installed files: {e}"))?;
                after_park(index + 1)?;
            }
            state.installed.retain(|r| r.app_id != id);
            state.retained.retain(|r| r.app_id != id);
            self.commit(&state)
        })();
        if let Err(error) = operation {
            return match self.recover_uninstall() {
                Ok(()) => Err(error),
                Err(recovery) => Err(format!("{error}; uninstall recovery pending: {recovery}")),
            };
        }
        // A committed uninstall has no active receipt. Recovery verifies each parked inventory
        // again before deletion, and retains its journal if cleanup is interrupted.
        before_cleanup()?;
        self.recover_uninstall()
    }

    fn verify_receipt(&self, receipt: &Receipt) -> Result<(), String> {
        let root = self.version_path(receipt)?;
        self.verify_inventory(&root, receipt)
    }

    fn verify_inventory(&self, root: &Path, receipt: &Receipt) -> Result<(), String> {
        if receipt.files.is_empty() {
            return Err("receipt has no verified file inventory".into());
        }
        guard_dir(root)?;
        let expected: HashSet<_> = receipt
            .files
            .iter()
            .map(|f| f.path.to_ascii_lowercase())
            .collect();
        let expected_dirs = parent_directories(&receipt.files);
        let mut actual = HashSet::new();
        let mut actual_dirs = HashSet::new();
        scan_files(root, root, &mut actual, &mut actual_dirs)?;
        if expected != actual || expected_dirs != actual_dirs {
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

    fn verify_remaining_inventory(&self, root: &Path, receipt: &Receipt) -> Result<(), String> {
        guard_dir(root)?;
        let mut actual = HashSet::new();
        let mut actual_dirs = HashSet::new();
        scan_files(root, root, &mut actual, &mut actual_dirs)?;
        let expected: HashSet<_> = receipt
            .files
            .iter()
            .map(|file| file.path.to_ascii_lowercase())
            .collect();
        if !actual.is_subset(&expected)
            || !actual_dirs.is_subset(&parent_directories(&receipt.files))
        {
            return Err("uninstall parking contains an unowned file".into());
        }
        for file in &receipt.files {
            if actual.contains(&file.path.to_ascii_lowercase()) {
                let path = root.join(&file.path);
                guard_file(&path)?;
                let (bytes, sha) = file_digest(&path)?;
                if bytes != file.bytes || sha != file.sha256 {
                    return Err("uninstall parking contains a changed file".into());
                }
            }
        }
        Ok(())
    }
}

fn approved_redirect(source: &Url, next: &Url) -> bool {
    if next.scheme() != "https"
        || next.port().is_some()
        || !next.username().is_empty()
        || next.password().is_some()
        || next.fragment().is_some()
    {
        return false;
    }
    if next.host_str() == Some("github.com") {
        return next.query().is_none() && next.path().eq_ignore_ascii_case(source.path());
    }
    matches!(
        next.host_str(),
        Some(
            "release-assets.githubusercontent.com"
                | "objects.githubusercontent.com"
                | "github-releases.githubusercontent.com"
        )
    )
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
    let entry_count = zip.len();
    for index in 0..entry_count {
        if !progress(Phase::Extracting) {
            return Err("installation cancelled".into());
        }
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        let name = entry.name().trim_end_matches('/').to_string();
        let destination = root.join(&name);
        if entry.is_dir() {
            fs::create_dir_all(&destination).map_err(|e| e.to_string())?;
            guard_dir(&destination)?;
            if !progress(Phase::ExtractedEntries(index + 1, entry_count)) {
                return Err("installation cancelled".into());
            }
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
        if !progress(Phase::ExtractedEntries(index + 1, entry_count)) {
            return Err("installation cancelled".into());
        }
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

fn parent_directories(files: &[InstalledFile]) -> HashSet<String> {
    let mut dirs = HashSet::new();
    for file in files {
        let mut parent = file.path.as_str();
        while let Some((prefix, _)) = parent.rsplit_once('/') {
            dirs.insert(prefix.to_ascii_lowercase());
            parent = prefix;
        }
    }
    dirs
}

fn scan_files(
    root: &Path,
    path: &Path,
    found: &mut HashSet<String>,
    dirs: &mut HashSet<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if is_link(&metadata) {
            return Err("installed directory contains a link".into());
        }
        if metadata.is_dir() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if !dirs.insert(relative.to_ascii_lowercase()) {
                return Err("installed directory contains case-colliding directories".into());
            }
            scan_files(root, &path, found, dirs)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            if !found.insert(relative.to_ascii_lowercase()) {
                return Err("installed directory contains case-colliding files".into());
            }
        } else {
            return Err("installed directory contains a special file".into());
        }
    }
    Ok(())
}

fn path_occupied(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("inspect install path: {error}")),
    }
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

#[cfg(test)]
mod redirect_tests {
    use super::*;

    #[test]
    fn permits_only_same_release_casing_redirect_or_github_asset_host() {
        let source =
            Url::parse("https://github.com/Gerbzz/gridstrike/releases/download/v1/app.zip")
                .unwrap();
        let canonical =
            Url::parse("https://github.com/Gerbzz/GridStrike/releases/download/v1/app.zip")
                .unwrap();
        let asset = Url::parse("https://release-assets.githubusercontent.com/signed-asset?token=1")
            .unwrap();
        let other =
            Url::parse("https://github.com/SomeoneElse/gridstrike/releases/download/v1/app.zip")
                .unwrap();
        assert!(approved_redirect(&source, &canonical));
        assert!(approved_redirect(&source, &asset));
        assert!(!approved_redirect(&source, &other));
    }
}

#[cfg(test)]
mod uninstall_transaction_tests {
    use super::*;
    use tempfile::TempDir;
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn fixture() -> (TempDir, InstallRoot) {
        let tmp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
        let root = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
        let app_dir = root.app_path("gridstrike").unwrap();
        fs::create_dir_all(&app_dir).unwrap();
        let mut receipts = Vec::new();
        for (version, bytes) in [("1.0.0", b"one".as_slice()), ("2.0.0", b"two".as_slice())] {
            let version_dir = app_dir.join(version);
            fs::create_dir(&version_dir).unwrap();
            fs::write(version_dir.join("gridstrike.exe"), bytes).unwrap();
            receipts.push(Receipt {
                app_id: "gridstrike".into(),
                version: version.into(),
                platform: std::env::consts::OS.into(),
                arch: std::env::consts::ARCH.into(),
                entrypoint: "gridstrike.exe".into(),
                files: vec![InstalledFile {
                    path: "gridstrike.exe".into(),
                    bytes: bytes.len() as u64,
                    sha256: format!("{:x}", Sha256::digest(bytes)),
                }],
            });
        }
        root.commit(&InstalledState {
            schema: crate::STATE_SCHEMA,
            installed: vec![receipts.remove(1)],
            retained: receipts,
        })
        .unwrap();
        fs::write(root.path.join("user-project.txt"), b"keep").unwrap();
        (tmp, root)
    }

    fn reinstall_package(tmp: &TempDir) -> (PathBuf, crate::Release) {
        let path = tmp.path().join("reinstall.zip");
        let mut zip = ZipWriter::new(File::create(&path).unwrap());
        zip.start_file("gridstrike.exe", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"fresh").unwrap();
        zip.finish().unwrap();
        let bytes = fs::read(&path).unwrap();
        (
            path,
            crate::Release {
                app_id: "gridstrike".into(),
                version: "2.0.0".into(),
                platform: std::env::consts::OS.into(),
                arch: std::env::consts::ARCH.into(),
                artifact_url: String::new(),
                artifact_sha256: format!("{:x}", Sha256::digest(&bytes)),
                artifact_bytes: bytes.len() as u64,
                entrypoint: "gridstrike.exe".into(),
            },
        )
    }

    fn park_current_before_commit(root: &InstallRoot) -> Receipt {
        let receipt = root.state().unwrap().receipt("gridstrike").unwrap().clone();
        let parked_name = ".uninstall-interrupted";
        root.write_uninstall_journal(&UninstallJournal {
            schema: 1,
            app_id: "gridstrike".into(),
            versions: vec![ParkedVersion {
                receipt: receipt.clone(),
                parked_name: parked_name.into(),
            }],
        })
        .unwrap();
        fs::rename(
            root.version_path(&receipt).unwrap(),
            root.app_path("gridstrike").unwrap().join(parked_name),
        )
        .unwrap();
        receipt
    }

    #[test]
    fn failure_after_first_park_restores_every_version() {
        let (_tmp, root) = fixture();
        let error = root
            .uninstall_impl(
                "gridstrike",
                &mut |count| {
                    if count == 1 {
                        Err("injected parking failure".into())
                    } else {
                        Ok(())
                    }
                },
                &mut || Ok(()),
            )
            .unwrap_err();
        assert!(error.contains("injected parking failure"));
        assert_eq!(
            fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
            b"two"
        );
        assert_eq!(
            fs::read(root.path.join("apps/gridstrike/1.0.0/gridstrike.exe")).unwrap(),
            b"one"
        );
        assert!(!root.uninstall_journal_path().exists());
        assert_eq!(
            fs::read(root.path.join("user-project.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn uninstall_preserves_unowned_empty_directory() {
        let (_tmp, root) = fixture();
        let user_dir = root.path.join("apps/gridstrike/2.0.0/user-project");
        fs::create_dir(&user_dir).unwrap();
        assert!(root.uninstall("gridstrike").is_err());
        assert!(user_dir.is_dir());
        assert!(root.state().unwrap().receipt("gridstrike").is_some());
    }

    #[test]
    fn reopening_restores_versions_parked_before_receipt_commit() {
        let (tmp, root) = fixture();
        park_current_before_commit(&root);
        let parked = root
            .app_path("gridstrike")
            .unwrap()
            .join(".uninstall-interrupted");

        let reopened = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
        assert_eq!(
            fs::read(reopened.entrypoint("gridstrike").unwrap()).unwrap(),
            b"two"
        );
        assert!(!parked.exists());
        assert!(!reopened.uninstall_journal_path().exists());
        assert_eq!(
            fs::read(reopened.path.join("user-project.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn same_root_rollback_recovers_parked_current_version_first() {
        let (_tmp, root) = fixture();
        park_current_before_commit(&root);
        root.rollback("gridstrike").unwrap();
        assert!(!root.uninstall_journal_path().exists());
        assert_eq!(
            root.state().unwrap().receipt("gridstrike").unwrap().version,
            "1.0.0"
        );
        assert_eq!(
            fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
            b"one"
        );
        assert_eq!(
            fs::read(root.path.join("apps/gridstrike/2.0.0/gridstrike.exe")).unwrap(),
            b"two"
        );
    }

    #[test]
    fn same_root_launch_recovers_parked_receipt_before_starting_process() {
        let (_tmp, root) = fixture();
        let mut state = root.state().unwrap();
        state.retained.clear();
        root.commit(&state).unwrap();
        park_current_before_commit(&root);
        let error = root.launch("gridstrike", &[]).unwrap_err();
        assert!(error.contains("launch failed"), "{error}");
        assert!(!root.uninstall_journal_path().exists());
        assert_eq!(
            root.state().unwrap().receipt("gridstrike").unwrap().version,
            "2.0.0"
        );
        assert_eq!(
            fs::read(root.path.join("apps/gridstrike/2.0.0/gridstrike.exe")).unwrap(),
            b"two"
        );
    }

    #[test]
    fn reopening_finishes_verified_cleanup_after_receipt_commit() {
        let (tmp, root) = fixture();
        let state = root.state().unwrap();
        let app_dir = root.app_path("gridstrike").unwrap();
        let versions: Vec<_> = state
            .installed
            .iter()
            .chain(&state.retained)
            .enumerate()
            .map(|(index, receipt)| ParkedVersion {
                receipt: receipt.clone(),
                parked_name: format!(".uninstall-interrupted-{index}"),
            })
            .collect();
        root.write_uninstall_journal(&UninstallJournal {
            schema: 1,
            app_id: "gridstrike".into(),
            versions: versions.clone(),
        })
        .unwrap();
        for item in &versions {
            fs::rename(
                root.version_path(&item.receipt).unwrap(),
                app_dir.join(&item.parked_name),
            )
            .unwrap();
        }
        root.commit(&InstalledState::empty()).unwrap();

        let reopened = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
        assert!(reopened.state().unwrap().receipt("gridstrike").is_none());
        for item in &versions {
            assert!(!app_dir.join(&item.parked_name).exists());
        }
        assert!(!reopened.uninstall_journal_path().exists());
        assert_eq!(
            fs::read(reopened.path.join("user-project.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn reopening_finishes_partially_deleted_owned_parking() {
        let (tmp, root) = fixture();
        let receipt = root.state().unwrap().receipt("gridstrike").unwrap().clone();
        let parked_name = ".uninstall-partially-deleted";
        let parked = root.app_path("gridstrike").unwrap().join(parked_name);
        root.write_uninstall_journal(&UninstallJournal {
            schema: 1,
            app_id: "gridstrike".into(),
            versions: vec![ParkedVersion {
                receipt: receipt.clone(),
                parked_name: parked_name.into(),
            }],
        })
        .unwrap();
        fs::rename(root.version_path(&receipt).unwrap(), &parked).unwrap();
        root.commit(&InstalledState::empty()).unwrap();
        fs::remove_file(parked.join("gridstrike.exe")).unwrap();

        let reopened = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
        assert!(!parked.exists());
        assert!(!reopened.uninstall_journal_path().exists());
        assert_eq!(
            fs::read(reopened.path.join("user-project.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn same_root_reinstall_finishes_partially_failed_committed_uninstall() {
        let (tmp, root) = fixture();
        let error = root
            .uninstall_impl("gridstrike", &mut |_| Ok(()), &mut || {
                let journal = root.read_uninstall_journal()?.unwrap();
                let parked = root
                    .app_path("gridstrike")?
                    .join(&journal.versions.last().unwrap().parked_name);
                fs::remove_dir_all(parked).map_err(|e| e.to_string())?;
                Err("injected partial cleanup failure".into())
            })
            .unwrap_err();
        assert!(error.contains("injected partial cleanup failure"));
        assert!(root.state().unwrap().receipt("gridstrike").is_none());
        assert!(root.uninstall_journal_path().exists());

        let (archive, release) = reinstall_package(&tmp);
        root.install_archive(
            &InstallPlan {
                release: &release,
                is_update: false,
            },
            &archive,
            &mut |_| true,
        )
        .unwrap();
        assert!(!root.uninstall_journal_path().exists());
        assert_eq!(
            fs::read(root.entrypoint("gridstrike").unwrap()).unwrap(),
            b"fresh"
        );
        let reopened = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
        assert_eq!(
            fs::read(reopened.entrypoint("gridstrike").unwrap()).unwrap(),
            b"fresh"
        );
        assert_eq!(
            fs::read(reopened.path.join("user-project.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn same_root_blocks_reinstall_while_committed_cleanup_contains_unowned_data() {
        let (tmp, root) = fixture();
        let error = root
            .uninstall_impl("gridstrike", &mut |_| Ok(()), &mut || {
                let journal = root.read_uninstall_journal()?.unwrap();
                let parked = root
                    .app_path("gridstrike")?
                    .join(&journal.versions[0].parked_name);
                fs::write(parked.join("user-project.txt"), b"keep").map_err(|e| e.to_string())?;
                Err("injected cleanup failure".into())
            })
            .unwrap_err();
        assert!(error.contains("injected cleanup failure"));
        let (archive, release) = reinstall_package(&tmp);
        let install = root.install_archive(
            &InstallPlan {
                release: &release,
                is_update: false,
            },
            &archive,
            &mut |_| true,
        );
        assert!(install.is_err(), "install must wait for journal recovery");
        assert!(root.state().unwrap().receipt("gridstrike").is_none());
        assert!(!root.path.join("apps/gridstrike/2.0.0").exists());
        assert!(root.uninstall_journal_path().exists());
        assert!(InstallRoot::open(&tmp.path().join("launcher")).is_err());
        let journal = root.read_uninstall_journal().unwrap().unwrap();
        let parked = root
            .app_path("gridstrike")
            .unwrap()
            .join(&journal.versions[0].parked_name);
        assert_eq!(fs::read(parked.join("user-project.txt")).unwrap(), b"keep");
        fs::remove_file(parked.join("user-project.txt")).unwrap();
        let reopened = InstallRoot::open(&tmp.path().join("launcher")).unwrap();
        assert!(!reopened.uninstall_journal_path().exists());
        assert!(reopened.state().unwrap().receipt("gridstrike").is_none());
    }
}
