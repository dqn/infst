//! Windows registry helpers for INFINITAS URL handler management.

use std::path::Path;

use anyhow::{Context, Result, bail};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CLASSES_ROOT, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_OPTION_NON_VOLATILE,
    REG_SZ, RegCloseKey, RegCreateKeyExW, RegEnumKeyExW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW,
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
pub fn register_url_handler(exe_path: &Path, asio: bool) -> Result<()> {
    let exe_str = exe_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Executable path is not valid UTF-8"))?;

    let no_asio_flag = if asio { "" } else { " --no-asio" };
    let command_value = format!(r#""{exe_str}" launch run{no_asio_flag} "%1""#);

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

const XONAR_KEY_NAME: &str = "XONAR SOUND CARD(64)";
const ASIO_SUBKEY: &str = r"SOFTWARE\ASIO";

/// Register a spoofed "XONAR SOUND CARD(64)" ASIO device by cloning the CLSID
/// from an existing ASIO driver. INFINITAS only recognizes the Xonar device name.
pub fn setup_asio_spoof(asio_device: Option<&str>) -> Result<()> {
    let drivers = enumerate_asio_drivers()?;

    if drivers.is_empty() {
        bail!("No ASIO drivers found in registry. Install an ASIO driver first (e.g. ASIO4ALL)");
    }

    // Check if Xonar entry already exists
    if let Some(existing) = drivers.iter().find(|d| d.name == XONAR_KEY_NAME) {
        println!(
            "ASIO spoof already configured: {} (CLSID: {})",
            existing.name, existing.clsid
        );
        return Ok(());
    }

    // Pick the source driver
    let source = match asio_device {
        Some(name) => drivers.iter().find(|d| d.name == name).ok_or_else(|| {
            let available: Vec<&str> = drivers.iter().map(|d| d.name.as_str()).collect();
            anyhow::anyhow!(
                "ASIO device '{}' not found. Available: {}",
                name,
                available.join(", ")
            )
        })?,
        None if drivers.len() == 1 => &drivers[0],
        None => prompt_asio_driver(&drivers)?,
    };

    println!(
        "Creating ASIO spoof: {} -> {} (CLSID: {})",
        source.name, XONAR_KEY_NAME, source.clsid
    );

    let xonar_subkey = format!(r"{ASIO_SUBKEY}\{XONAR_KEY_NAME}");
    unsafe {
        let key = create_key(HKEY_LOCAL_MACHINE, &xonar_subkey)?;
        let _guard = KeyGuard(key);
        set_string_value(key, Some("CLSID"), &source.clsid)?;
        set_string_value(key, Some("Description"), XONAR_KEY_NAME)?;
    }

    Ok(())
}

/// Interactively prompt the user to select an ASIO driver.
fn prompt_asio_driver(drivers: &[AsioDriver]) -> Result<&AsioDriver> {
    println!("Select ASIO driver to use:");
    for (i, d) in drivers.iter().enumerate() {
        println!("  [{}] {}", i + 1, d.name);
    }
    print!("Enter number (1-{}): ", drivers.len());
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let index: usize = input
        .trim()
        .parse::<usize>()
        .ok()
        .and_then(|n| n.checked_sub(1))
        .filter(|&i| i < drivers.len())
        .ok_or_else(|| anyhow::anyhow!("Invalid selection: {}", input.trim()))?;

    Ok(&drivers[index])
}

/// Remove the spoofed XONAR ASIO device entry.
pub fn remove_asio_spoof() -> Result<()> {
    let xonar_subkey = HSTRING::from(format!(r"{ASIO_SUBKEY}\{XONAR_KEY_NAME}"));
    unsafe {
        // Try to delete; ignore if it doesn't exist
        let result =
            windows::Win32::System::Registry::RegDeleteKeyW(HKEY_LOCAL_MACHINE, &xonar_subkey);
        if result.is_ok() {
            println!("Removed ASIO spoof: {XONAR_KEY_NAME}");
        }
    }
    Ok(())
}

struct AsioDriver {
    name: String,
    clsid: String,
}

/// Enumerate all ASIO drivers from HKLM\SOFTWARE\ASIO.
fn enumerate_asio_drivers() -> Result<Vec<AsioDriver>> {
    let subkey = HSTRING::from(ASIO_SUBKEY);
    let mut drivers = Vec::new();

    unsafe {
        let mut asio_key = HKEY::default();
        let res = RegOpenKeyExW(HKEY_LOCAL_MACHINE, &subkey, 0, KEY_READ, &mut asio_key);
        if res.is_err() {
            return Ok(drivers); // No ASIO key at all
        }
        let _guard = KeyGuard(asio_key);

        let mut index: u32 = 0;
        loop {
            let mut name_buf = [0u16; 256];
            let mut name_len = name_buf.len() as u32;
            let res = RegEnumKeyExW(
                asio_key,
                index,
                windows::core::PWSTR(name_buf.as_mut_ptr()),
                &mut name_len,
                None,
                windows::core::PWSTR::null(),
                None,
                None,
            );
            if res.is_err() {
                break;
            }

            let name = String::from_utf16_lossy(&name_buf[..name_len as usize]);

            // Read CLSID from subkey
            if let Ok(clsid) = read_string_value_under(
                HKEY_LOCAL_MACHINE,
                &format!(r"{ASIO_SUBKEY}\{name}"),
                "CLSID",
            ) {
                drivers.push(AsioDriver { name, clsid });
            }

            index += 1;
        }
    }

    Ok(drivers)
}

/// Read a REG_SZ value from a subkey.
fn read_string_value_under(root: HKEY, subkey: &str, value_name: &str) -> Result<String> {
    let subkey_h = HSTRING::from(subkey);
    let value_h = HSTRING::from(value_name);

    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(root, &subkey_h, 0, KEY_READ, &mut hkey)
            .ok()
            .with_context(|| format!("Failed to open key: {subkey}"))?;
        let _guard = KeyGuard(hkey);

        let mut size: u32 = 0;
        RegQueryValueExW(hkey, &value_h, None, None, None, Some(&mut size))
            .ok()
            .with_context(|| format!("Failed to query {value_name} size"))?;

        let mut buffer = vec![0u8; size as usize];
        RegQueryValueExW(
            hkey,
            &value_h,
            None,
            None,
            Some(buffer.as_mut_ptr()),
            Some(&mut size),
        )
        .ok()
        .with_context(|| format!("Failed to read {value_name}"))?;

        let wide: Vec<u16> = buffer
            .chunks_exact(2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .collect();
        Ok(String::from_utf16_lossy(&wide)
            .trim_end_matches('\0')
            .to_string())
    }
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
