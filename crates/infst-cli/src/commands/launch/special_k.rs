//! Special K global injection setup for INFINITAS.
//!
//! Uses SKIF (Special K Injection Frontend) for global injection instead of
//! local DLL proxy. Configures the Special K profile for borderless fullscreen
//! at 120fps.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

const PROFILE_NAME: &str = "beatmania IIDX INFINITAS";

/// Settings to apply for borderless 120fps mode.
const BORDERLESS_SETTINGS: &[(&str, &str, &str)] = &[
    ("Window.System", "Borderless", "true"),
    ("Window.System", "Fullscreen", "true"),
    ("Window.System", "Center", "true"),
    ("Window.System", "ConfineCursor", "true"),
    ("Display.Output", "ForceWindowed", "true"),
    ("Display.Output", "ForceFullscreen", "false"),
    ("Render.FrameRate", "TargetFPS", "120.0"),
    ("Render.FrameRate", "PresentationInterval", "0"),
    ("Render.FrameRate", "SleeplessRenderThread", "true"),
    ("Steam.Log", "Silent", "true"),
];

/// Special K installation directory under %LOCALAPPDATA%.
fn sk_install_dir() -> Result<PathBuf> {
    let local_data = dirs::data_local_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine LOCALAPPDATA directory"))?;
    let sk_dir = local_data.join("Programs").join("Special K");
    if !sk_dir.exists() {
        bail!(
            "Special K not found at: {}\n\
             Install Special K (SKIF) from https://www.special-k.info/",
            sk_dir.display()
        );
    }
    Ok(sk_dir)
}

/// Configure Special K profile for borderless 120fps and clean up old local injection files.
pub fn install(game_dir: &Path) -> Result<()> {
    let sk_dir = sk_install_dir()?;
    let profile_dir = sk_dir.join("Profiles").join(PROFILE_NAME);
    let ini_path = profile_dir.join("SpecialK.ini");

    if ini_path.exists() {
        let content = read_utf16_file(&ini_path)
            .with_context(|| format!("Failed to read {}", ini_path.display()))?;
        let updated = update_ini_settings(&content, BORDERLESS_SETTINGS);
        write_utf16_file(&ini_path, &updated)
            .with_context(|| format!("Failed to write {}", ini_path.display()))?;
        println!("Updated Special K profile for borderless 120fps");
    } else {
        bail!(
            "Special K profile not found: {}\n\
             Start SKIF, add INFINITAS to its game list, and launch the game once first.",
            ini_path.display()
        );
    }

    // Clean up old local injection files (from previous versions)
    for name in ["dxgi.dll", "dxgi.ini"] {
        let path = game_dir.join(name);
        if path.exists() && std::fs::remove_file(&path).is_ok() {
            println!("Removed old {}", path.display());
        }
    }

    Ok(())
}

/// Remove borderless settings from Special K profile and clean up old local injection files.
pub fn uninstall(game_dir: &Path) -> Result<()> {
    // Clean up old local injection files
    for name in ["dxgi.dll", "dxgi.ini"] {
        let path = game_dir.join(name);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            println!("Removed {}", path.display());
        }
    }

    // Reset profile settings
    let sk_dir = sk_install_dir()?;
    let ini_path = sk_dir
        .join("Profiles")
        .join(PROFILE_NAME)
        .join("SpecialK.ini");

    if ini_path.exists() {
        let content = read_utf16_file(&ini_path)?;
        let reset = &[
            ("Window.System", "Borderless", "false"),
            ("Window.System", "Fullscreen", "false"),
            ("Window.System", "Center", "false"),
            ("Window.System", "ConfineCursor", "false"),
            ("Display.Output", "ForceWindowed", "false"),
            ("Render.FrameRate", "TargetFPS", "0.000000"),
            ("Render.FrameRate", "PresentationInterval", "-1"),
            ("Render.FrameRate", "SleeplessRenderThread", "false"),
        ];
        let updated = update_ini_settings(&content, reset);
        write_utf16_file(&ini_path, &updated)?;
        println!("Reset Special K profile settings");
    }

    Ok(())
}

/// Ensure SKIF is running for global injection.
pub fn ensure_running() -> Result<()> {
    if is_skif_running() {
        return Ok(());
    }

    let sk_dir = sk_install_dir()?;
    let skif = sk_dir.join("SKIF.exe");
    if !skif.exists() {
        bail!("SKIF.exe not found at: {}", skif.display());
    }

    println!("Starting SKIF...");
    std::process::Command::new(&skif).spawn()?;
    std::thread::sleep(std::time::Duration::from_secs(3));
    Ok(())
}

fn is_skif_running() -> bool {
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq SKIF.exe", "/NH"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("SKIF.exe"))
        .unwrap_or(false)
}

/// Read a UTF-16 LE file (with optional BOM) into a String.
fn read_utf16_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let content = if bytes.starts_with(&[0xFF, 0xFE]) {
        &bytes[2..]
    } else {
        &bytes[..]
    };
    let utf16: Vec<u16> = content
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&utf16).context("Invalid UTF-16 in Special K profile")
}

/// Write a String as UTF-16 LE with BOM.
fn write_utf16_file(path: &Path, content: &str) -> Result<()> {
    let mut bytes = vec![0xFF_u8, 0xFE];
    for unit in content.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(path, bytes).context("Failed to write file")
}

/// Update specific key=value pairs in INI content, grouped by section.
fn update_ini_settings(content: &str, settings: &[(&str, &str, &str)]) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();

    for &(section, key, value) in settings {
        let header = format!("[{}]", section);
        let mut in_section = false;
        let mut found = false;

        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                in_section = trimmed == header;
            } else if in_section
                && let Some(eq) = trimmed.find('=')
                && trimmed[..eq].trim() == key
            {
                *line = format!("{}={}", key, value);
                found = true;
                break;
            }
        }

        if !found {
            // Find the section to insert after, or create it
            let mut insert_at = None;
            let mut in_target = false;
            for (i, line) in lines.iter().enumerate() {
                let trimmed = line.trim();
                if trimmed == header {
                    in_target = true;
                    insert_at = Some(i + 1);
                } else if in_target {
                    if trimmed.starts_with('[') {
                        break;
                    }
                    insert_at = Some(i + 1);
                }
            }
            if let Some(idx) = insert_at {
                lines.insert(idx, format!("{}={}", key, value));
            } else {
                lines.push(String::new());
                lines.push(header);
                lines.push(format!("{}={}", key, value));
            }
        }
    }

    lines.join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_ini_settings_modifies_existing() {
        let content = "[Window.System]\r\nBorderless=false\r\nFullscreen=false\r\n";
        let settings = &[
            ("Window.System", "Borderless", "true"),
            ("Window.System", "Fullscreen", "true"),
        ];
        let result = update_ini_settings(content, settings);
        assert!(result.contains("Borderless=true"));
        assert!(result.contains("Fullscreen=true"));
    }

    #[test]
    fn test_update_ini_settings_adds_missing_key() {
        let content = "[Window.System]\r\nBorderless=false\r\n";
        let settings = &[("Window.System", "Center", "true")];
        let result = update_ini_settings(content, settings);
        assert!(result.contains("Center=true"));
    }

    #[test]
    fn test_update_ini_settings_adds_missing_section() {
        let content = "[Window.System]\r\nBorderless=false\r\n";
        let settings = &[("Display.Output", "ForceWindowed", "true")];
        let result = update_ini_settings(content, settings);
        assert!(result.contains("[Display.Output]"));
        assert!(result.contains("ForceWindowed=true"));
    }

    #[test]
    fn test_update_ini_settings_does_not_cross_sections() {
        let content = "[Section.A]\r\nKey=old\r\n\r\n[Section.B]\r\nKey=keep\r\n";
        let settings = &[("Section.A", "Key", "new")];
        let result = update_ini_settings(content, settings);
        // Section.A's Key should be updated
        let section_a_start = result.find("[Section.A]").unwrap();
        let section_b_start = result.find("[Section.B]").unwrap();
        let between = &result[section_a_start..section_b_start];
        assert!(between.contains("Key=new"));
        // Section.B's Key should be unchanged
        let after_b = &result[section_b_start..];
        assert!(after_b.contains("Key=keep"));
    }
}
