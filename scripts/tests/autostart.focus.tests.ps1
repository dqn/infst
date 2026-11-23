param([string]$WatcherPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'autostart.ps1'))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class InfstFocusTest {
    public delegate void WinEventDelegate(IntPtr hook, uint evt, IntPtr window, int obj, int child, uint thread, uint time);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] static extern IntPtr SetWinEventHook(uint first, uint last, IntPtr module, WinEventDelegate callback, uint process, uint thread, uint flags);
    [DllImport("user32.dll")] public static extern bool UnhookWinEvent(IntPtr hook);
    public static readonly List<long> Activations = new List<long>();
    static readonly WinEventDelegate Callback = (h, e, w, o, c, t, ms) => Activations.Add(w.ToInt64());
    public static IntPtr Watch() { return SetWinEventHook(3, 3, IntPtr.Zero, Callback, 0, 0, 0); }
}
'@
$root = Join-Path $env:TEMP ('infst-focus-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$gameExe = Join-Path $root 'infst-focus-game.exe'
$trackerExe = Join-Path $root 'infst-focus-tracker.exe'
Copy-Item "$env:WINDIR\System32\cmd.exe" $gameExe
Copy-Item "$env:WINDIR\System32\cmd.exe" $trackerExe
$game = $null
$hook = [IntPtr]::Zero
try {
    $game = Start-Process $gameExe -ArgumentList '/k' -WindowStyle Hidden -PassThru
    $foreground = [InfstFocusTest]::GetForegroundWindow()
    if ($foreground -eq [IntPtr]::Zero) { throw 'Run the focus regression test on an unlocked Windows desktop' }
    $hook = [InfstFocusTest]::Watch()
    if ($hook -eq [IntPtr]::Zero) { throw 'Cannot watch foreground activation events' }
    $common = @{ GameProcessName = 'infst-focus-game'; TrackerProcessName = 'infst-focus-tracker'; TrackerPath = $trackerExe; WorkingDirectory = $root; TrackerArguments = @('/k'); LogPath = (Join-Path $root 'autostart.log'); Once = $true }
    & $WatcherPath @common
    $until = [DateTime]::UtcNow.AddSeconds(2)
    while ([DateTime]::UtcNow -lt $until) {
        [System.Windows.Forms.Application]::DoEvents()
        Start-Sleep -Milliseconds 10
    }
    $trackers = @(Get-Process infst-focus-tracker -ErrorAction SilentlyContinue)
    if ($trackers.Count -ne 1) { throw 'Did not start exactly one tracker for the game' }
    $changes = @([InfstFocusTest]::Activations | Where-Object { $_ -ne $foreground.ToInt64() })
    if ($changes.Count -gt 0 -or [InfstFocusTest]::GetForegroundWindow() -ne $foreground) {
        throw ('Tracker startup interrupted the foreground game window; activation events=' + ($changes -join ','))
    }
    Write-Output 'PASS: tracker starts without any foreground activation change'
} finally {
    if ($hook -ne [IntPtr]::Zero) { [InfstFocusTest]::UnhookWinEvent($hook) | Out-Null }
    Get-Process infst-focus-tracker -ErrorAction SilentlyContinue | Where-Object Path -EQ $trackerExe | Stop-Process
    if ($game -and !$game.HasExited) { Stop-Process -Id $game.Id -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
