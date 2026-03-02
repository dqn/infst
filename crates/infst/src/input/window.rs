//! Game window management.
//!
//! Locates the INFINITAS window by process ID, manages foreground focus,
//! and provides borderless window mode transformation.

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::HWND;

/// Find the main window belonging to the given process ID.
///
/// Enumerates all top-level windows and returns the first one whose owning
/// process matches `target_pid`.
#[cfg(target_os = "windows")]
pub fn find_window_by_pid(target_pid: u32) -> anyhow::Result<HWND> {
    use std::sync::Mutex;
    use windows::Win32::Foundation::LPARAM;
    use windows::Win32::UI::WindowsAndMessaging::EnumWindows;

    // Shared state for the enum callback
    let found: Mutex<Option<HWND>> = Mutex::new(None);
    let _found_ref = &found;
    let pid = target_pid;

    // SAFETY: EnumWindows calls the callback for each top-level window.
    // The callback checks the owning PID and visibility.
    unsafe {
        // We pass pid via the LPARAM so the callback can access it.
        EnumWindows(Some(enum_callback), LPARAM(&pid as *const u32 as isize)).ok();
    }

    // The static callback below writes into a thread-local; we read it here.
    let hwnd = FOUND_HWND.with(|cell| cell.take());

    hwnd.ok_or_else(|| anyhow::anyhow!("No visible window found for PID {}", target_pid))
}

#[cfg(target_os = "windows")]
thread_local! {
    static FOUND_HWND: std::cell::Cell<Option<HWND>> = const { std::cell::Cell::new(None) };
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_callback(
    hwnd: HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::BOOL {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};

    let target_pid = unsafe { *(lparam.0 as *const u32) };
    let mut window_pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut window_pid)) };

    if window_pid == target_pid && unsafe { IsWindowVisible(hwnd) }.as_bool() {
        FOUND_HWND.with(|cell| cell.set(Some(hwnd)));
        return BOOL(0); // Stop enumeration
    }
    BOOL(1) // Continue enumeration
}

/// Bring the given window to the foreground.
#[cfg(target_os = "windows")]
pub fn ensure_foreground(hwnd: HWND) -> anyhow::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

    // SAFETY: SetForegroundWindow is safe to call with a valid HWND.
    // It may fail silently if the calling process doesn't have permission,
    // but this is harmless.
    unsafe {
        let _ = SetForegroundWindow(hwnd);
    }
    Ok(())
}

/// Check whether the given window currently has foreground focus.
#[cfg(target_os = "windows")]
pub fn is_foreground(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    // SAFETY: GetForegroundWindow is always safe to call.
    let fg = unsafe { GetForegroundWindow() };
    fg == hwnd
}

/// Decoration flags that borderless mode strips.
#[cfg(target_os = "windows")]
const DECORATION_STYLE_FLAGS: u32 = {
    use windows::Win32::UI::WindowsAndMessaging::{
        WS_CAPTION, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_SYSMENU, WS_THICKFRAME,
    };
    WS_CAPTION.0 | WS_THICKFRAME.0 | WS_SYSMENU.0 | WS_MAXIMIZEBOX.0 | WS_MINIMIZEBOX.0
};

/// Check whether the window is already in borderless mode.
///
/// Returns `true` if no decoration flags are set. Non-decoration flags like
/// `WS_CLIPSIBLINGS` are ignored since the game may re-apply them.
#[cfg(target_os = "windows")]
pub fn is_borderless(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongPtrW};

    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32;
    (style & DECORATION_STYLE_FLAGS) == 0
}

/// Apply borderless window mode: strip all decorations and resize to fill the monitor.
///
/// Removes both standard and extended window styles (Borderless-Gaming approach),
/// then repositions the window to cover the entire monitor.
/// Uses `SWP_NOSENDCHANGING` to bypass the game's `WM_WINDOWPOSCHANGING` handler
/// which restricts window resizing.
///
/// Returns `true` if styles were modified, `false` if already borderless.
#[cfg(target_os = "windows")]
pub fn apply_borderless(hwnd: HWND) -> anyhow::Result<bool> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_EXSTYLE, GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOOWNERZORDER,
        SWP_NOSENDCHANGING, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, WINDOW_EX_STYLE,
        WINDOW_STYLE, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_STATICEDGE, WS_EX_WINDOWEDGE,
    };

    // SAFETY: GetWindowLongPtrW reads window style bits.
    let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
    let ex_style = WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32);

    eprintln!("  style: 0x{:08X}, ex_style: 0x{:08X}", style.0, ex_style.0);

    // Skip if already borderless (no decoration flags remain)
    let border_ex_flags =
        WS_EX_DLGMODALFRAME | WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_STATICEDGE;
    if (style.0 & DECORATION_STYLE_FLAGS) == 0 && (ex_style & border_ex_flags).0 == 0 {
        eprintln!("  Already borderless, skipping");
        return Ok(false);
    }

    // Subtract decoration flags, preserve non-decoration flags like WS_CLIPSIBLINGS
    let new_style = WINDOW_STYLE(style.0 & !DECORATION_STYLE_FLAGS);

    // Strip border-related extended styles (Borderless-Gaming approach)
    let new_ex_style =
        ex_style & !WS_EX_DLGMODALFRAME & !WS_EX_WINDOWEDGE & !WS_EX_CLIENTEDGE & !WS_EX_STATICEDGE;

    eprintln!(
        "  -> style: 0x{:08X}, ex_style: 0x{:08X}",
        new_style.0, new_ex_style.0
    );

    // SAFETY: SetWindowLongPtrW updates window style bits.
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, new_style.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, new_ex_style.0 as isize);
    }

    let rect = get_monitor_rect(hwnd)?;

    // SAFETY: SetWindowPos repositions and resizes the window to fill the monitor.
    // SWP_SHOWWINDOW forces display, SWP_NOOWNERZORDER prevents owned window Z-order changes,
    // SWP_NOSENDCHANGING bypasses game's WM_WINDOWPOSCHANGING handler.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_SHOWWINDOW | SWP_NOOWNERZORDER | SWP_FRAMECHANGED | SWP_NOSENDCHANGING,
        )?;
    }

    Ok(true)
}

/// Apply DWM optimizations to reduce composition overhead for a windowed game.
///
/// - Disables DWM transition animations
/// - Disables non-client area rendering (already stripped for borderless)
/// - Disables peek preview to avoid extra frame capture
#[cfg(target_os = "windows")]
pub fn apply_dwm_optimizations(hwnd: HWND) -> anyhow::Result<()> {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Graphics::Dwm::{
        DWMWA_DISALLOW_PEEK, DWMWA_EXCLUDED_FROM_PEEK, DWMWA_NCRENDERING_POLICY,
        DWMWA_TRANSITIONS_FORCEDISABLED, DwmSetWindowAttribute,
    };

    let true_val = BOOL(1);
    let nc_disabled: u32 = 1; // DWMNCRP_DISABLED

    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &true_val as *const BOOL as *const _,
            size_of::<BOOL>() as u32,
        )?;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &nc_disabled as *const u32 as *const _,
            size_of::<u32>() as u32,
        )?;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_DISALLOW_PEEK,
            &true_val as *const BOOL as *const _,
            size_of::<BOOL>() as u32,
        )?;
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_EXCLUDED_FROM_PEEK,
            &true_val as *const BOOL as *const _,
            size_of::<BOOL>() as u32,
        )?;
    }

    Ok(())
}

/// Get the monitor rectangle for the monitor containing the given window.
#[cfg(target_os = "windows")]
fn get_monitor_rect(hwnd: HWND) -> anyhow::Result<windows::Win32::Foundation::RECT> {
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };

    // SAFETY: MonitorFromWindow returns the monitor handle for the window.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    // SAFETY: GetMonitorInfoW fills the MONITORINFO struct for a valid monitor handle.
    let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
    if !ok.as_bool() {
        anyhow::bail!("GetMonitorInfoW failed");
    }

    Ok(info.rcMonitor)
}

// --- Non-Windows stubs ---

#[cfg(not(target_os = "windows"))]
pub fn find_window_by_pid(_target_pid: u32) -> anyhow::Result<()> {
    anyhow::bail!("Window management is only supported on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_foreground(_hwnd: ()) -> anyhow::Result<()> {
    anyhow::bail!("Window management is only supported on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn is_foreground(_hwnd: ()) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn is_borderless(_hwnd: ()) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub fn apply_borderless(_hwnd: ()) -> anyhow::Result<bool> {
    anyhow::bail!("Window management is only supported on Windows")
}

#[cfg(not(target_os = "windows"))]
pub fn apply_dwm_optimizations(_hwnd: ()) -> anyhow::Result<()> {
    anyhow::bail!("Window management is only supported on Windows")
}
