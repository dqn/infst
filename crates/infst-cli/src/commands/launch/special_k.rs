//! Special K DLL and INI management for INFINITAS.

use std::path::Path;

use anyhow::{Context, Result, bail};

/// Default Special K install location under %LOCALAPPDATA%.
fn default_special_k_path() -> Result<std::path::PathBuf> {
    let local_data = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine LOCALAPPDATA directory"))?;
    Ok(local_data
        .join("Programs")
        .join("Special K")
        .join("SpecialK64.dll"))
}

/// Borderless + 120fps config for Special K.
const DXGI_INI: &str = r#"[Render.FrameRate]
TargetFPS=120.0
SleeplessRender=true
PresentationInterval=0

[Window.System]
Borderless=true

[Steam.Log]
Silent=true
"#;

/// Install Special K DLL and config into the INFINITAS game directory.
pub fn install(game_dir: &Path, custom_sk_path: Option<&str>) -> Result<()> {
    let sk_dll = match custom_sk_path {
        Some(p) => std::path::PathBuf::from(p),
        None => default_special_k_path()?,
    };

    if !sk_dll.exists() {
        bail!(
            "SpecialK64.dll not found at: {}\n\
             Install Special K or specify the path with --special-k-path",
            sk_dll.display()
        );
    }

    let dest_dll = game_dir.join("dxgi.dll");
    let dest_ini = game_dir.join("dxgi.ini");

    std::fs::copy(&sk_dll, &dest_dll).with_context(|| {
        format!(
            "Failed to copy {} -> {}",
            sk_dll.display(),
            dest_dll.display()
        )
    })?;
    println!("Copied SpecialK64.dll -> {}", dest_dll.display());

    std::fs::write(&dest_ini, DXGI_INI)
        .with_context(|| format!("Failed to write {}", dest_ini.display()))?;
    println!("Wrote dxgi.ini -> {}", dest_ini.display());

    Ok(())
}

/// Remove Special K DLL and config from the INFINITAS game directory.
pub fn uninstall(game_dir: &Path) -> Result<()> {
    let dll_path = game_dir.join("dxgi.dll");
    let ini_path = game_dir.join("dxgi.ini");

    if dll_path.exists() {
        std::fs::remove_file(&dll_path)
            .with_context(|| format!("Failed to remove {}", dll_path.display()))?;
        println!("Removed {}", dll_path.display());
    }

    if ini_path.exists() {
        std::fs::remove_file(&ini_path)
            .with_context(|| format!("Failed to remove {}", ini_path.display()))?;
        println!("Removed {}", ini_path.display());
    }

    Ok(())
}
