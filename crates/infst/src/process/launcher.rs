//! Game launcher — registry lookup, token extraction, and direct game launch.

/// Extract the authentication token from an INFINITAS URI.
///
/// The URI contains a `tk=` parameter followed by a 64-character hex token.
/// Matches the pattern from inf_launch_ext: `$Args[0] -match "tk=(.{64})"`.
pub fn extract_token_from_uri(uri: &str) -> anyhow::Result<String> {
    let marker = "tk=";
    let pos = uri
        .find(marker)
        .ok_or_else(|| anyhow::anyhow!("URI does not contain 'tk=' parameter"))?;

    let start = pos + marker.len();
    let remaining = &uri[start..];

    if remaining.len() < 64 {
        anyhow::bail!(
            "Token too short: expected 64 characters, found {}",
            remaining.len()
        );
    }

    // Take exactly 64 characters (stop at & if present, but token should be 64 chars)
    let token: String = remaining.chars().take(64).collect();
    if token.len() < 64 {
        anyhow::bail!(
            "Token too short: expected 64 characters, found {}",
            token.len()
        );
    }

    Ok(token)
}

/// Find the game executable path from the Windows registry.
///
/// Reads `HKLM\SOFTWARE\KONAMI\beatmania IIDX INFINITAS\InstallDir`
/// and returns `{InstallDir}\game\app\bm2dx.exe`.
#[cfg(target_os = "windows")]
pub fn find_game_executable() -> anyhow::Result<std::path::PathBuf> {
    use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::HSTRING;

    let subkey = HSTRING::from(r"SOFTWARE\KONAMI\beatmania IIDX INFINITAS");
    let value_name = HSTRING::from("InstallDir");

    // First call to get the required buffer size
    let mut size: u32 = 0;
    // SAFETY: RegGetValueW with null buffer queries the required size.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &subkey,
            &value_name,
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
        .ok()
        .map_err(|e| anyhow::anyhow!("Failed to query registry value size: {e}"))?;
    }

    // Allocate buffer and read the value
    let mut buffer = vec![0u16; (size as usize) / 2];
    // SAFETY: RegGetValueW reads the registry value into the provided buffer.
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            &subkey,
            &value_name,
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .map_err(|e| anyhow::anyhow!("Failed to read registry value: {e}"))?;
    }

    // Trim null terminator
    if buffer.last() == Some(&0) {
        buffer.pop();
    }

    let install_dir = String::from_utf16(&buffer)
        .map_err(|e| anyhow::anyhow!("Invalid UTF-16 in registry value: {e}"))?;

    let exe_path = std::path::PathBuf::from(install_dir)
        .join("game")
        .join("app")
        .join("bm2dx.exe");

    if !exe_path.exists() {
        anyhow::bail!("Game executable not found at: {}", exe_path.display());
    }

    Ok(exe_path)
}

#[cfg(not(target_os = "windows"))]
pub fn find_game_executable() -> anyhow::Result<std::path::PathBuf> {
    anyhow::bail!("Game executable lookup is only supported on Windows")
}

/// Launch the game directly with the given token in windowed mode.
///
/// Runs `bm2dx.exe -t TOKEN -w` and returns the child process ID.
#[cfg(target_os = "windows")]
pub fn launch_game(token: &str) -> anyhow::Result<u32> {
    let exe = find_game_executable()?;
    let child = std::process::Command::new(&exe)
        .args(["-t", token, "-w"])
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to launch game: {e}"))?;

    Ok(child.id())
}

#[cfg(not(target_os = "windows"))]
pub fn launch_game(_token: &str) -> anyhow::Result<u32> {
    anyhow::bail!("Game launching is only supported on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_token_valid_uri() {
        let uri = "bm2dxinf://something?tk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&other=param";
        let token = extract_token_from_uri(uri).unwrap();
        assert_eq!(token.len(), 64);
        assert_eq!(token, "A".repeat(64));
    }

    #[test]
    fn extract_token_at_end_of_uri() {
        let uri = format!("bm2dxinf://launch?tk={}", "B".repeat(64));
        let token = extract_token_from_uri(&uri).unwrap();
        assert_eq!(token, "B".repeat(64));
    }

    #[test]
    fn extract_token_missing_tk() {
        let uri = "bm2dxinf://something?foo=bar";
        let result = extract_token_from_uri(uri);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("tk="));
    }

    #[test]
    fn extract_token_too_short() {
        let uri = "bm2dxinf://something?tk=tooshort";
        let result = extract_token_from_uri(uri);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too short"));
    }

    #[test]
    fn extract_token_exactly_64_chars() {
        let token_str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        assert_eq!(token_str.len(), 64);
        let uri = format!("bm2dxinf://x?tk={token_str}");
        let token = extract_token_from_uri(&uri).unwrap();
        assert_eq!(token, token_str);
    }
}
