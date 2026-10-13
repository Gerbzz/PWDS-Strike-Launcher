//! Shared catalog, installed-state view, and fail-closed release planning.
//! There is intentionally no downloader, archive extractor, or install executor yet.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use url::Url;

pub const STATE_SCHEMA: u32 = 1;
pub const RELEASE_SCHEMA: u32 = 1;
const MAX_STATE_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug)]
pub struct App {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    /// A source repository is not evidence of a distributable release.
    pub github_repo: Option<&'static str>,
}

pub const APPS: &[App] = &[
    App {
        id: "clipstrike",
        name: "ClipStrike",
        category: "Video",
        description: "Edit and finish CS2 footage",
        github_repo: Some("clipstrike"),
    },
    App {
        id: "demostrike",
        name: "DemoStrike",
        category: "CS2",
        description: "Analyze demos and prepare captures",
        github_repo: Some("demostrike"),
    },
    App {
        id: "streamstrike",
        name: "StreamStrike",
        category: "Video",
        description: "Recording and streaming project foundation",
        github_repo: None,
    },
    App {
        id: "audiostrike",
        name: "AudioStrike",
        category: "Audio",
        description: "Audio creation and editing",
        github_repo: Some("audiostrike"),
    },
    App {
        id: "gridstrike",
        name: "GridStrike",
        category: "Office",
        description: "Spreadsheet workspace",
        github_repo: Some("gridstrike"),
    },
    App {
        id: "notestrike",
        name: "NoteStrike",
        category: "Office",
        description: "Notes and writing",
        github_repo: None,
    },
    App {
        id: "pdfstrike",
        name: "PDFStrike",
        category: "Office",
        description: "PDF workspace",
        github_repo: None,
    },
    App {
        id: "skinstrike",
        name: "SkinStrike",
        category: "CS2",
        description: "Weapon finish design",
        github_repo: None,
    },
    App {
        id: "mapstrike",
        name: "MapStrike",
        category: "CS2",
        description: "Map project tools",
        github_repo: None,
    },
    App {
        id: "modelstrike",
        name: "ModelStrike",
        category: "3D",
        description: "Prop and model preparation",
        github_repo: None,
    },
    App {
        id: "hungerstrike",
        name: "HungerStrike",
        category: "Games",
        description: "Local game prototype",
        github_repo: None,
    },
];

pub fn app(id: &str) -> Option<&'static App> {
    APPS.iter().find(|item| item.id == id)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifest {
    pub schema: u32,
    pub channel: String,
    pub releases: Vec<Release>,
}

/// Can only be constructed from release metadata embedded at build time.
pub struct TrustedManifest(ReleaseManifest);

impl TrustedManifest {
    pub fn releases(&self) -> &[Release] {
        &self.0.releases
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub app_id: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub artifact_url: String,
    pub artifact_sha256: String,
    pub artifact_bytes: u64,
    pub entrypoint: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledState {
    pub schema: u32,
    pub installed: Vec<Receipt>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub app_id: String,
    pub version: String,
    pub platform: String,
    pub arch: String,
    pub entrypoint: String,
}

impl InstalledState {
    pub fn empty() -> Self {
        Self {
            schema: STATE_SCHEMA,
            installed: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Self::empty()),
            Err(e) => return Err(format!("cannot open installed state: {e}")),
        };
        if file.metadata().map_err(|e| e.to_string())?.len() > MAX_STATE_BYTES {
            return Err("installed state exceeds 1 MiB".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_STATE_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_STATE_BYTES {
            return Err("installed state exceeds 1 MiB".into());
        }
        let state: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("invalid installed state: {e}"))?;
        state.validate()?;
        Ok(state)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != STATE_SCHEMA {
            return Err("unsupported installed-state schema".into());
        }
        let mut ids = HashSet::new();
        for item in &self.installed {
            if app(&item.app_id).is_none() || !ids.insert(&item.app_id) {
                return Err("unknown or duplicate installed app ID".into());
            }
            version_key(&item.version)?;
            validate_target(&item.platform, &item.arch)?;
            validate_entrypoint(&item.entrypoint)?;
        }
        Ok(())
    }

    pub fn receipt(&self, id: &str) -> Option<&Receipt> {
        self.installed.iter().find(|item| item.app_id == id)
    }
}

impl Default for InstalledState {
    fn default() -> Self {
        Self::empty()
    }
}

/// The bundled manifest is compiled into the launcher. No remote manifest URL is trusted.
pub fn bundled_manifest() -> Result<TrustedManifest, String> {
    parse_bundled_manifest(include_str!("../releases.json"))
}

fn parse_bundled_manifest(json: &str) -> Result<TrustedManifest, String> {
    let manifest: ReleaseManifest =
        serde_json::from_str(json).map_err(|e| format!("invalid bundled manifest: {e}"))?;
    validate_manifest(&manifest)?;
    Ok(TrustedManifest(manifest))
}

fn validate_manifest(manifest: &ReleaseManifest) -> Result<(), String> {
    if manifest.schema != RELEASE_SCHEMA || manifest.channel != "stable" {
        return Err("unsupported release schema or channel".into());
    }
    let mut identities = HashSet::new();
    for release in &manifest.releases {
        validate_release(release)?;
        let identity = (
            &release.app_id,
            &release.version,
            &release.platform,
            &release.arch,
        );
        if !identities.insert(identity) {
            return Err("duplicate release identity".into());
        }
    }
    Ok(())
}

fn validate_release(release: &Release) -> Result<(), String> {
    let app = app(&release.app_id).ok_or("unknown release app ID")?;
    version_key(&release.version)?;
    validate_target(&release.platform, &release.arch)?;
    validate_entrypoint(&release.entrypoint)?;
    if release.artifact_bytes == 0 || release.artifact_bytes > 8 * 1024 * 1024 * 1024 {
        return Err("artifact size is outside supported limits".into());
    }
    if release.artifact_sha256.len() != 64
        || !release
            .artifact_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit())
    {
        return Err("artifact SHA-256 must be 64 hex digits".into());
    }
    let repo = app
        .github_repo
        .ok_or("app has no approved release repository")?;
    let url = Url::parse(&release.artifact_url).map_err(|_| "invalid artifact URL")?;
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("artifact URL is not a trusted GitHub release URL".into());
    }
    let parts: Vec<_> = url
        .path_segments()
        .ok_or("invalid artifact URL path")?
        .collect();
    if parts.len() != 6
        || parts[0] != "Gerbzz"
        || parts[1] != repo
        || parts[2] != "releases"
        || parts[3] != "download"
        || parts[4].is_empty()
        || parts[5].is_empty()
        || !parts[5].ends_with(".zip")
    {
        return Err("artifact URL is not an approved repository release ZIP".into());
    }
    for part in [parts[4], parts[5]] {
        if part == "."
            || part == ".."
            || !part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err("release URL tag or filename contains unsafe characters".into());
        }
    }
    Ok(())
}

fn version_key(value: &str) -> Result<(u32, u32, u32), String> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() != 3 {
        return Err("version must be numeric major.minor.patch".into());
    }
    let mut numbers = [0u32; 3];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty()
            || (part.len() > 1 && part.starts_with('0'))
            || !part.bytes().all(|b| b.is_ascii_digit())
        {
            return Err("invalid version component".into());
        }
        numbers[index] = part.parse().map_err(|_| "version component out of range")?;
    }
    Ok((numbers[0], numbers[1], numbers[2]))
}

fn validate_target(platform: &str, arch: &str) -> Result<(), String> {
    if !matches!(platform, "windows" | "linux" | "macos") || !matches!(arch, "x86_64" | "aarch64") {
        return Err("unsupported platform or architecture".into());
    }
    Ok(())
}

pub fn current_target() -> (&'static str, &'static str) {
    (std::env::consts::OS, std::env::consts::ARCH)
}

/// ZIP entrypoints are portable slash-separated relative paths. Filesystem extraction
/// must independently reject symlinks, junctions, hard links and path escapes.
pub fn validate_entrypoint(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > 240 || value.contains('\\') {
        return Err("unsafe entrypoint path".into());
    }
    for part in value.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.ends_with(' ')
            || part.ends_with('.')
            || !part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        {
            return Err("unsafe entrypoint path".into());
        }
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if matches!(
            stem.as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        ) {
            return Err("reserved Windows path component".into());
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct InstallPlan<'a> {
    pub release: &'a Release,
    pub is_update: bool,
}

/// Computes a plan only. It never downloads, extracts, launches or changes state.
pub fn plan_install<'a>(
    manifest: &'a TrustedManifest,
    state: &InstalledState,
    id: &str,
    platform: &str,
    arch: &str,
) -> Result<InstallPlan<'a>, String> {
    if app(id).is_none() {
        return Err("unknown app ID".into());
    }
    state.validate()?;
    validate_target(platform, arch)?;
    let current = state.receipt(id);
    if current.is_some_and(|receipt| receipt.platform != platform || receipt.arch != arch) {
        return Err("installed receipt belongs to another platform".into());
    }
    let mut matches: Vec<_> = manifest
        .releases()
        .iter()
        .filter(|release| {
            release.app_id == id && release.platform == platform && release.arch == arch
        })
        .collect();
    matches.sort_by_key(|release| version_key(&release.version).expect("validated release"));
    let release = matches
        .pop()
        .ok_or("No verified release for this platform; coming soon")?;
    if let Some(receipt) = current {
        if version_key(&release.version)? <= version_key(&receipt.version)? {
            return Err("No newer verified release is available".into());
        }
    }
    Ok(InstallPlan {
        release,
        is_update: current.is_some(),
    })
}

pub fn availability(
    manifest: &TrustedManifest,
    state: &InstalledState,
    id: &str,
    platform: &str,
    arch: &str,
) -> &'static str {
    if let Some(receipt) = state.receipt(id) {
        if receipt.platform != platform || receipt.arch != arch {
            return "Installed for another platform";
        }
        if plan_install(manifest, state, id, platform, arch).is_ok() {
            "Installed · update planned"
        } else {
            "Installed"
        }
    } else if plan_install(manifest, state, id, platform, arch).is_ok() {
        "Verified release · installer planned"
    } else {
        "Coming soon · no verified release"
    }
}

/// Inspect a downloaded artifact before any extraction or activation.
pub fn verify_artifact(path: &Path, release: &Release) -> Result<(), String> {
    if !std::fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_file()
    {
        return Err("artifact is not a regular file".into());
    }
    let mut file = File::open(path).map_err(|e| format!("cannot open artifact: {e}"))?;
    let mut hasher = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        count = count
            .checked_add(n as u64)
            .ok_or("artifact byte count overflow")?;
        if count > release.artifact_bytes {
            return Err("artifact exceeds pinned byte length".into());
        }
        hasher.update(&buf[..n]);
    }
    if count != release.artifact_bytes
        || format!("{:x}", hasher.finalize()) != release.artifact_sha256.to_ascii_lowercase()
    {
        return Err("artifact does not match pinned size and SHA-256".into());
    }
    Ok(())
}

pub fn default_state_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        return std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("PWDS")
            .join("StrikeLauncher")
            .join("installed.json");
    }
    #[cfg(target_os = "macos")]
    {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("Library/Application Support/PWDS/StrikeLauncher/installed.json");
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|x| PathBuf::from(x).join(".local/state")))
            .unwrap_or_else(std::env::temp_dir)
            .join("pwds/strike-launcher/installed.json")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_catalog_has_no_fake_release() {
        let manifest = bundled_manifest().unwrap();
        assert!(manifest.releases().is_empty());
        for app in APPS {
            assert!(!app.name.starts_with("PWDS "));
        }
    }

    #[test]
    fn path_and_origin_checks_fail_closed() {
        for bad in [
            "../bad.exe",
            "/bad.exe",
            "C:/bad.exe",
            "dir\\bad.exe",
            "CON.exe",
            "a//b.exe",
            "a/./b.exe",
        ] {
            assert!(validate_entrypoint(bad).is_err(), "{bad}");
        }
        let mut release = Release {
            app_id: "clipstrike".into(),
            version: "1.2.3".into(),
            platform: "windows".into(),
            arch: "x86_64".into(),
            artifact_url:
                "https://github.com/Gerbzz/clipstrike/releases/download/v1.2.3/clipstrike.zip"
                    .into(),
            artifact_sha256: "a".repeat(64),
            artifact_bytes: 10,
            entrypoint: "bin/clipstrike.exe".into(),
        };
        assert!(validate_release(&release).is_ok());
        release.artifact_url =
            "https://github.com.evil.test/Gerbzz/clipstrike/releases/download/v1/x.zip".into();
        assert!(validate_release(&release).is_err());
    }

    #[test]
    fn no_release_means_no_install_plan() {
        let state = InstalledState::empty();
        let manifest = bundled_manifest().unwrap();
        assert!(plan_install(&manifest, &state, "clipstrike", "windows", "x86_64").is_err());
        assert!(
            availability(&manifest, &state, "clipstrike", "windows", "x86_64")
                .contains("Coming soon")
        );
    }
}
