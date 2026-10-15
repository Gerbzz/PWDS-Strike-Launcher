use pwds_strike_launcher::{
    APPS, app, availability, bundled_manifest, current_target, default_state_path,
    installer::{InstallRoot, Phase},
    plan_install,
};

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("catalog");
    #[cfg(feature = "developer-preview")]
    if command.starts_with("preview-") {
        return preview(&args);
    }
    let root_path = default_state_path()
        .parent()
        .ok_or("state path has no parent")?
        .to_path_buf();
    let root = InstallRoot::open(&root_path)?;
    let manifest = bundled_manifest()?;
    let state = root.state()?;
    let (platform, arch) = current_target();
    println!("PWDS Strike Launcher - {platform}/{arch}");
    match (command, args.as_slice()) {
        ("catalog", [] | [_]) => {
            for item in APPS { println!("{:<16} {:<20} {}", item.id, item.name, availability(&manifest, &state, item.id, platform, arch)); }
        }
        ("library", [_]) => {
            if state.installed.is_empty() { println!("No launcher-managed apps installed."); }
            for receipt in &state.installed {
                let item = app(&receipt.app_id).ok_or("unknown receipt app")?;
                println!("{} {} ({}/{})", item.name, receipt.version, receipt.platform, receipt.arch);
            }
        }
        ("plan", [_, id]) => {
            let plan = plan_install(&manifest, &state, id, platform, arch)?;
            println!("{} {}: {}", if plan.is_update { "Update" } else { "Install" }, plan.release.version, plan.release.artifact_url);
            println!("Pinned bytes: {} - SHA-256: {}", plan.release.artifact_bytes, plan.release.artifact_sha256);
        }
        ("install", [_, id]) => {
            let plan = plan_install(&manifest, &state, id, platform, arch)?;
            root.download_and_install(&plan, &mut |phase| { print_phase(phase); true })?;
            println!("Installed {} {}", id, plan.release.version);
        }
        ("launch", [_, id]) => { println!("Started process {}", root.launch(id, &[])?); }
        ("uninstall", [_, id]) => { root.uninstall(id)?; println!("Uninstalled {id}; documents and settings preserved."); }
        ("rollback", [_, id]) => { root.rollback(id)?; println!("Restored previous verified version of {id}."); }
        _ => return Err("usage: pwds-strike-launcher [catalog | library | plan <app-id> | install <app-id> | launch <app-id> | rollback <app-id> | uninstall <app-id>]".into()),
    }
    Ok(())
}

fn print_phase(phase: Phase) {
    match phase {
        Phase::DownloadBytes(done, total) => eprintln!("Downloading: {done}/{total} bytes"),
        other => eprintln!("{other:?}"),
    }
}

#[cfg(feature = "developer-preview")]
fn preview(args: &[String]) -> Result<(), String> {
    use pwds_strike_launcher::{InstallPlan, Release};
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::Read, path::Path};
    eprintln!("DEVELOPMENT PREVIEW - local GridStrike package, not a published release");
    match args {
        [command, version, archive, root] if command == "preview-install" => {
            let mut file = File::open(archive).map_err(|e| e.to_string())?;
            let mut hash = Sha256::new();
            let mut bytes = 0u64;
            let mut buf = [0u8; 64 * 1024];
            loop { let n = file.read(&mut buf).map_err(|e| e.to_string())?; if n == 0 { break; } bytes += n as u64; hash.update(&buf[..n]); }
            let (platform, arch) = current_target();
            let release = Release { app_id: "gridstrike".into(), version: version.clone(), platform: platform.into(), arch: arch.into(), artifact_url: String::new(), artifact_sha256: format!("{:x}", hash.finalize()), artifact_bytes: bytes, entrypoint: "gridstrike.exe".into() };
            let root = InstallRoot::open(Path::new(root))?;
            let is_update = root.state()?.receipt("gridstrike").is_some();
            root.install_archive(&InstallPlan { release: &release, is_update }, Path::new(archive), &mut |phase| { print_phase(phase); true })?;
            println!("Development preview installed: GridStrike {version}");
        }
        [command, root] if command == "preview-launch-version" => {
            let root = InstallRoot::open(Path::new(root))?;
            let pid = root.launch("gridstrike", &["--version".into()])?;
            println!("Started GridStrike version process {pid}");
        }
        [command, root] if command == "preview-uninstall" => {
            InstallRoot::open(Path::new(root))?.uninstall("gridstrike")?;
            println!("Development preview uninstalled; user data preserved.");
        }
        _ => return Err("development preview usage: preview-install <version> <local-zip> <isolated-root> | preview-launch-version <isolated-root> | preview-uninstall <isolated-root>".into()),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PWDS Strike Launcher: {error}");
        std::process::exit(1);
    }
}
