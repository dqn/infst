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

/// Check whether the given window is minimised (iconic).
#[cfg(target_os = "windows")]
pub fn is_minimized(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::IsIconic;

    // SAFETY: IsIconic is safe to call with any HWND.
    unsafe { IsIconic(hwnd).as_bool() }
}

#[cfg(not(target_os = "windows"))]
pub fn is_minimized(_hwnd: ()) -> bool {
    false
}

/// Apply borderless window mode: strip decorations and resize to fill the monitor.
///
/// Sets the window style to `WS_VISIBLE` only (matching infzoom's approach),
/// then repositions the window to cover the entire monitor using `HWND_TOPMOST`
/// so it stays in front of other windows.
///
/// Flags match infzoom's `ResizeWindow()`:
/// - `SWP_NOSENDCHANGING` — bypass the game's `WM_WINDOWPOSCHANGING` handler
/// - `SWP_ASYNCWINDOWPOS` — don't block on the move
/// - `SWP_NOCOPYBITS` / `SWP_NOREDRAW` — let the game handle its own rendering
#[cfg(target_os = "windows")]
pub fn apply_borderless(hwnd: HWND) -> anyhow::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, HWND_TOPMOST, SWP_ASYNCWINDOWPOS, SWP_NOCOPYBITS, SWP_NOREDRAW,
        SWP_NOSENDCHANGING, SetWindowLongPtrW, SetWindowPos, WS_VISIBLE,
    };

    // Strip all decorations, keep only WS_VISIBLE (matches infzoom)
    // SAFETY: SetWindowLongPtrW with GWL_STYLE updates window style bits.
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, WS_VISIBLE.0 as isize);
    }

    let rect = get_monitor_rect(hwnd)?;

    // SAFETY: SetWindowPos repositions and resizes the window to fill the monitor.
    // HWND_TOPMOST keeps the window above all non-topmost windows.
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_ASYNCWINDOWPOS | SWP_NOCOPYBITS | SWP_NOREDRAW | SWP_NOSENDCHANGING,
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
pub fn apply_borderless(_hwnd: ()) -> anyhow::Result<()> {
    anyhow::bail!("Window management is only supported on Windows")
}
