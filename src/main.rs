use pwds_strike_launcher::{
    APPS, InstalledState, app, availability, bundled_manifest, current_target, default_state_path,
    plan_install,
};

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("catalog");
    if !matches!(command, "catalog" | "library" | "plan")
        || (command == "plan" && args.len() != 2)
        || (command != "plan" && args.len() > 1)
    {
        return Err("usage: pwds-strike-launcher [catalog | library | plan <app-id>]".into());
    }
    let manifest = bundled_manifest()?;
    let state = InstalledState::load(&default_state_path())?;
    let (platform, arch) = current_target();
    println!("PWDS Strike Launcher · {platform}/{arch}");
    match command {
        "catalog" => {
            for item in APPS {
                println!(
                    "{:<16} {:<20} {}",
                    item.id,
                    item.name,
                    availability(&manifest, &state, item.id, platform, arch)
                );
            }
        }
        "library" => {
            if state.installed.is_empty() {
                println!("No launcher-managed apps installed.");
            }
            for receipt in &state.installed {
                let item = app(&receipt.app_id).ok_or("unknown receipt app")?;
                println!(
                    "{} {} ({}/{})",
                    item.name, receipt.version, receipt.platform, receipt.arch
                );
            }
        }
        "plan" => {
            let plan = plan_install(&manifest, &state, &args[1], platform, arch)?;
            println!(
                "{} {}: {}",
                if plan.is_update {
                    "Update plan"
                } else {
                    "Install plan"
                },
                plan.release.version,
                plan.release.artifact_url
            );
            println!(
                "Pinned bytes: {} · SHA-256: {}",
                plan.release.artifact_bytes, plan.release.artifact_sha256
            );
            println!("Planning only; no installation executor exists yet.");
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PWDS Strike Launcher: {error}");
        std::process::exit(1);
    }
}
