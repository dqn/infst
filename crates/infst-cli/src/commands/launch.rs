//! Launch command — start INFINITAS in borderless window mode.

use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use infst::ProcessHandle;
use infst::input::window;
use infst::launcher;

const LOGIN_PAGE_URL: &str = "https://p.eagate.573.jp/game/2dx/infinitas/top/index.html";
const WINDOW_POLL_INTERVAL: Duration = Duration::from_millis(500);
const WINDOW_POLL_TIMEOUT: Duration = Duration::from_secs(60);
const PROCESS_POLL_INTERVAL: Duration = Duration::from_secs(2);
const BORDERLESS_RETRY_DELAY: Duration = Duration::from_secs(1);
const BORDERLESS_MAX_RETRIES: u32 = 3;

pub fn run(url: Option<&str>, pid: Option<u32>, timeout_secs: u64, windowed: bool) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");

    if windowed {
        eprintln!("infst {} - Launch (Windowed Borderless)", current_version);
    } else {
        eprintln!("infst {} - Launch (FSO Borderless)", current_version);
    }

    let process = find_or_launch_process(url, pid, timeout_secs, windowed)?;
    eprintln!("Game process found (PID: {})", process.pid);

    if windowed {
        wait_and_apply_borderless(&process)?;
    }

    eprintln!("Done!");
    Ok(())
}

/// Find a running game process, or launch one and wait for it.
fn find_or_launch_process(
    url: Option<&str>,
    pid: Option<u32>,
    timeout_secs: u64,
    windowed: bool,
) -> Result<ProcessHandle> {
    // Explicit PID — just open it
    if let Some(pid) = pid {
        return Ok(ProcessHandle::open(pid)?);
    }

    // Already running — use it
    if let Ok(process) = ProcessHandle::find_and_open() {
        eprintln!("Game is already running");
        return Ok(process);
    }

    // Not running — launch or instruct
    match url {
        Some(uri) => {
            let token = launcher::extract_token_from_uri(uri)?;

            if windowed {
                eprintln!("Launching game in windowed mode...");
            } else {
                // Ensure FSO is enabled before launching in fullscreen
                match launcher::ensure_fso_enabled() {
                    Ok(true) => eprintln!("Enabled Fullscreen Optimization for bm2dx.exe"),
                    Ok(false) => eprintln!("Fullscreen Optimization is already enabled"),
                    Err(e) => eprintln!("Warning: Could not check FSO status: {e}"),
                }
                eprintln!("Launching game in fullscreen mode (FSO borderless)...");
            }

            let pid = launcher::launch_game(&token, windowed)?;
            eprintln!("Game launched (PID: {})", pid);
        }
        None => {
            eprintln!("Game is not running. Opening login page...");
            open::that(LOGIN_PAGE_URL)?;
            eprintln!("Please log in and launch the game from the browser.");
        }
    }

    // Wait for the process to appear
    eprintln!("Waiting for game process (timeout: {}s)...", timeout_secs);
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    loop {
        if start.elapsed() > timeout {
            bail!("Timed out waiting for game process after {}s", timeout_secs);
        }

        if let Ok(process) = ProcessHandle::find_and_open() {
            return Ok(process);
        }

        thread::sleep(PROCESS_POLL_INTERVAL);
    }
}

/// Poll until a visible window appears for the process, then apply borderless mode.
///
/// Retries up to 3 times if the game re-applies window styles after modification
/// (Borderless-Gaming approach for games that fight back).
#[cfg(target_os = "windows")]
fn wait_and_apply_borderless(process: &ProcessHandle) -> Result<()> {
    eprintln!("Waiting for game window...");
    let start = Instant::now();

    let hwnd = loop {
        if start.elapsed() > WINDOW_POLL_TIMEOUT {
            bail!("Timed out waiting for game window");
        }

        if !process.is_alive() {
            bail!("Game process exited before a window appeared");
        }

        if let Ok(hwnd) = window::find_window_by_pid(process.pid) {
            break hwnd;
        }

        thread::sleep(WINDOW_POLL_INTERVAL);
    };

    eprintln!("Game window found");

    for attempt in 1..=BORDERLESS_MAX_RETRIES {
        eprintln!(
            "Applying borderless window mode (attempt {attempt}/{BORDERLESS_MAX_RETRIES})..."
        );

        let modified = window::apply_borderless(hwnd)?;
        if !modified {
            eprintln!("Window is already borderless");
            return Ok(());
        }

        // Wait and verify the style stuck
        thread::sleep(BORDERLESS_RETRY_DELAY);

        if window::is_borderless(hwnd) {
            eprintln!("Borderless mode applied successfully");
            return Ok(());
        }

        eprintln!("  Game reverted window styles, retrying...");
    }

    // Final attempt — apply without verification
    eprintln!("Applying borderless (final)...");
    window::apply_borderless(hwnd)?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn wait_and_apply_borderless(_process: &ProcessHandle) -> Result<()> {
    bail!("Borderless window mode is only supported on Windows");
}
