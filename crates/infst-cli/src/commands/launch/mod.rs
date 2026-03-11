//! Launch command: install/uninstall URL handler and Special K, run game from URL.

mod url;

#[cfg(target_os = "windows")]
mod registry;
#[cfg(target_os = "windows")]
mod special_k;

use anyhow::{Result, bail};

use crate::cli::LaunchAction;
use url::parse_launch_url;

pub fn run(action: LaunchAction) -> Result<()> {
    match action {
        LaunchAction::Install {
            special_k_path,
            no_asio,
            asio_device,
        } => install(special_k_path, !no_asio, asio_device),
        LaunchAction::Uninstall => uninstall(),
        LaunchAction::Run { url, no_asio } => run_game(&url, !no_asio),
    }
}

#[cfg(target_os = "windows")]
fn install(special_k_path: Option<String>, asio: bool, asio_device: Option<String>) -> Result<()> {
    let install_dir = registry::read_infinitas_install_dir()?;
    let game_dir = std::path::Path::new(&install_dir).join("game").join("app");

    println!("INFINITAS install dir: {install_dir}");

    // Set up Special K
    special_k::install(&game_dir, special_k_path.as_deref())?;

    // Set up ASIO device spoofing
    if asio {
        registry::setup_asio_spoof(asio_device.as_deref())?;
    }

    // Register URL handler (embed --asio flag if requested)
    let exe_path = std::env::current_exe()?;
    registry::register_url_handler(&exe_path, asio)?;

    println!("Installation complete!");
    println!("  - Special K DLL and config installed");
    if asio {
        println!("  - ASIO spoof device registered");
    }
    println!(
        "  - URL handler registered for bm2dxinf:// (ASIO: {})",
        if asio { "enabled" } else { "disabled" }
    );
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn install(
    _special_k_path: Option<String>,
    _asio: bool,
    _asio_device: Option<String>,
) -> Result<()> {
    bail!("Launch install is only supported on Windows");
}

#[cfg(target_os = "windows")]
fn uninstall() -> Result<()> {
    let install_dir = registry::read_infinitas_install_dir()?;
    let game_dir = std::path::Path::new(&install_dir).join("game").join("app");

    println!("INFINITAS install dir: {install_dir}");

    // Remove Special K files
    special_k::uninstall(&game_dir)?;

    // Remove ASIO spoof
    registry::remove_asio_spoof()?;

    // Restore original URL handler
    registry::restore_url_handler(&install_dir)?;

    println!("Uninstallation complete!");
    println!("  - Special K DLL and config removed");
    println!("  - ASIO spoof removed (if present)");
    println!("  - Original URL handler restored");
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn uninstall() -> Result<()> {
    bail!("Launch uninstall is only supported on Windows");
}

#[cfg(target_os = "windows")]
fn run_game(raw_url: &str, asio: bool) -> Result<()> {
    let params = parse_launch_url(raw_url)?;

    let install_dir = registry::read_infinitas_install_dir()?;
    let bm2dx_exe = std::path::Path::new(&install_dir)
        .join("game")
        .join("app")
        .join("bm2dx.exe");

    if !bm2dx_exe.exists() {
        bail!("bm2dx.exe not found at: {}", bm2dx_exe.display());
    }

    // No -w flag: game runs fullscreen, Special K converts to borderless 120fps
    let mut cmd = std::process::Command::new(&bm2dx_exe);
    if asio {
        cmd.arg("--asio");
    }
    cmd.arg("-t").arg(&params.token);
    if params.trial {
        cmd.arg("--trial");
    }

    println!(
        "Launching: {}{} -t <token>{}",
        bm2dx_exe.display(),
        if asio { " --asio" } else { "" },
        if params.trial { " --trial" } else { "" }
    );
    cmd.spawn()?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn run_game(_raw_url: &str, _asio: bool) -> Result<()> {
    bail!("Launch run is only supported on Windows");
}
