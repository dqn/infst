//! Windows registry helpers for INFINITAS URL handler management.

use std::path::Path;

use anyhow::{Context, Result};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE,
    REG_SZ, RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::core::HSTRING;

/// Read the INFINITAS install directory from the registry.
pub fn read_infinitas_install_dir() -> Result<String> {
    let subkey = HSTRING::from(r"SOFTWARE\KONAMI\beatmania IIDX INFINITAS");
    let value_name = HSTRING::from("InstallDir");

    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(HKEY_LOCAL_MACHINE, &subkey, 0, KEY_READ, &mut hkey)
            .ok()
            .context("Failed to open INFINITAS registry key. Is INFINITAS installed?")?;

        let _guard = KeyGuard(hkey);

        // Query size first
        let mut size: u32 = 0;
        RegQueryValueExW(hkey, &value_name, None, None, None, Some(&mut size))
            .ok()
            .context("Failed to query InstallDir size")?;

        // Read value
        let mut buffer = vec![0u8; size as usize];
        RegQueryValueExW(
            hkey,
            &value_name,
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        )
        .ok()
        .context("Failed to read InstallDir")?;

        // Convert wide string (trim null terminators)
        let wide: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        let path = String::from_utf16_lossy(&wide);
        Ok(path.trim_end_matches('\0').to_string())
    }
}

/// Register infst as the bm2dxinf:// URL handler.
pub fn register_url_handler(exe_path: &Path) -> Result<()> {
    let exe_str = exe_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Executable path is not valid UTF-8"))?;

    let command_value = format!(r#""{exe_str}" launch run "%1""#);

    unsafe {
        // Create/open HKCR\bm2dxinf
        let bm2dxinf_key = create_key(HKEY_CLASSES_ROOT, r"bm2dxinf")?;
        let _g1 = KeyGuard(bm2dxinf_key);

        set_string_value(bm2dxinf_key, None, "URL:beatmania IIDX INFINITAS")?;
        set_string_value(bm2dxinf_key, Some("URL Protocol"), "")?;

        // Create shell\open\command
        let command_key = create_key(HKEY_CLASSES_ROOT, r"bm2dxinf\shell\open\command")?;
        let _g2 = KeyGuard(command_key);

        set_string_value(command_key, None, &command_value)?;
    }

    Ok(())
}

/// Restore the original bm2dxinf:// URL handler to the KONAMI launcher.
pub fn restore_url_handler(install_dir: &str) -> Result<()> {
    let launcher = format!(
        r#""{}\launcher\modules\bm2dx_launcher.exe" "%1""#,
        install_dir.trim_end_matches('\\')
    );

    unsafe {
        let command_key = create_key(HKEY_CLASSES_ROOT, r"bm2dxinf\shell\open\command")?;
        let _guard = KeyGuard(command_key);
        set_string_value(command_key, None, &launcher)?;
    }

    Ok(())
}

/// Create or open a registry key.
unsafe fn create_key(root: HKEY, subkey: &str) -> Result<HKEY> {
    let subkey_h = HSTRING::from(subkey);
    let mut hkey = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            root,
            &subkey_h,
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        )
        .ok()
        .with_context(|| {
            format!("Failed to create registry key: {subkey}. Are you running as administrator?")
        })?;
    }
    Ok(hkey)
}

/// Set a REG_SZ value on an open registry key.
unsafe fn set_string_value(hkey: HKEY, name: Option<&str>, value: &str) -> Result<()> {
    let name_h = name.map(HSTRING::from);
    let name_ref = name_h.as_ref().map(|h| h as &HSTRING);

    // Encode as wide string with null terminator
    let wide: Vec<u16> = value.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes: Vec<u8> = wide.iter().flat_map(|w| w.to_le_bytes()).collect();

    unsafe {
        RegSetValueExW(
            hkey,
            name_ref.unwrap_or(&HSTRING::new()),
            0,
            REG_SZ,
            Some(&bytes),
        )
        .ok()
        .with_context(|| {
            format!(
                "Failed to set registry value: {}",
                name.unwrap_or("(Default)")
            )
        })?;
    }
    Ok(())
}

/// RAII guard to close a registry key.
struct KeyGuard(HKEY);

impl Drop for KeyGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}
