param(
    [Parameter(Mandatory)][string]$TrackerPath,
    [string]$WorkingDirectory = (Split-Path -Parent $TrackerPath),
    [string]$GameProcessName = 'bm2dx',
    [string]$TrackerProcessName = [IO.Path]::GetFileNameWithoutExtension($TrackerPath),
    [string[]]$TrackerArguments = @(),
    [int]$SessionId = (Get-Process -Id $PID).SessionId,
    [string]$LogPath = (Join-Path $PSScriptRoot 'autostart.log'),
    [switch]$Once
)
$ErrorActionPreference = 'Stop'
if (!(Test-Path -LiteralPath $TrackerPath -PathType Leaf)) { throw "Tracker not found: $TrackerPath" }
if (!(Test-Path -LiteralPath $WorkingDirectory -PathType Container)) { throw "Working directory not found: $WorkingDirectory" }
# A second logon/start request must not race the existing watcher.
$mutexName = 'Local\InfstAutoStart-' + $SessionId + '-' + $GameProcessName + '-' + $TrackerProcessName
$mutex = [Threading.Mutex]::new($false, $mutexName)
$ownsMutex = $false
try {
    try { $ownsMutex = $mutex.WaitOne(0) }
    catch [Threading.AbandonedMutexException] { $ownsMutex = $true }
    if (!$ownsMutex) { return }
    # Keep the tracker console available without activating it over the game.
    if (-not ('InfstAutoStartNative' -as [type])) {
        Add-Type @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class InfstAutoStartNative {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct StartupInfo {
        public int cb;
        public string reserved, desktop, title;
        public int x, y, width, height, xChars, yChars, fill, flags;
        public short showWindow, reservedSize;
        public IntPtr reservedBytes, input, output, error;
    }
    [StructLayout(LayoutKind.Sequential)]
    struct ProcessInfo {
        public IntPtr process, thread;
        public int processId, threadId;
    }
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    static extern bool CreateProcessW(string application, StringBuilder commandLine,
        IntPtr processSecurity, IntPtr threadSecurity, bool inheritHandles,
        uint flags, IntPtr environment, string directory, ref StartupInfo startup,
        out ProcessInfo process);
    [DllImport("kernel32.dll")]
    static extern bool CloseHandle(IntPtr handle);
    public static int StartMinimized(string application, string arguments, string directory) {
        var startup = new StartupInfo();
        startup.cb = Marshal.SizeOf(startup);
        startup.flags = 1; // STARTF_USESHOWWINDOW
        startup.showWindow = 7; // SW_SHOWMINNOACTIVE
        var commandLine = new StringBuilder("\"" + application + "\"" +
            (String.IsNullOrEmpty(arguments) ? "" : " " + arguments));
        ProcessInfo process;
        if (!CreateProcessW(application, commandLine, IntPtr.Zero, IntPtr.Zero,
            false, 0x10, IntPtr.Zero, directory, ref startup, out process))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        CloseHandle(process.thread);
        CloseHandle(process.process);
        return process.processId;
    }
}
'@
    }
    $seen = @()
    while ($true) {
        $games = @(Get-Process -Name $GameProcessName -ErrorAction SilentlyContinue | Where-Object SessionId -EQ $SessionId)
        foreach ($game in $games) {
            if ($seen -contains $game.Id) { continue }
            $existing = @(Get-Process -Name $TrackerProcessName -ErrorAction SilentlyContinue | Where-Object SessionId -EQ $SessionId)
            if ($existing.Count -eq 0) {
                try {
                    $trackerId = [InfstAutoStartNative]::StartMinimized($TrackerPath, ($TrackerArguments -join ' '), $WorkingDirectory)
                    Add-Content -LiteralPath $LogPath -Value ("{0:o} Started tracker PID={1} for game PID={2}, session={3}" -f (Get-Date), $trackerId, $game.Id, $SessionId)
                } catch {
                    Add-Content -LiteralPath $LogPath -Value ("{0:o} Tracker start failed: {1}" -f (Get-Date), $_.Exception.Message)
                    if ($Once) { throw }
                }
            }
        }
        $seen = @($games | Select-Object -ExpandProperty Id)
        if ($Once) { break }
        Start-Sleep -Seconds 3
    }
} finally {
    if ($ownsMutex) { $mutex.ReleaseMutex() }
    $mutex.Dispose()
}
