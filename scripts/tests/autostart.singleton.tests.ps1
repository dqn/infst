param([string]$WatcherPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'autostart.ps1'))
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:TEMP ('infst-singleton-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$gameExe = Join-Path $root 'infst-singleton-game.exe'
$trackerExe = Join-Path $root 'infst-singleton-tracker.exe'
Copy-Item "$env:WINDIR\System32\cmd.exe" $gameExe
Copy-Item "$env:WINDIR\System32\cmd.exe" $trackerExe
$watchers = @()
$game = $null
try {
    $arguments = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -File "{0}" -GameProcessName infst-singleton-game -TrackerProcessName infst-singleton-tracker -TrackerPath "{1}" -WorkingDirectory "{2}" -LogPath "{3}"' -f $WatcherPath, $trackerExe, $root, (Join-Path $root 'autostart.log')
    $watchers = @(1..2 | ForEach-Object {
        Start-Process "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList $arguments -WindowStyle Hidden -PassThru
    })
    Start-Sleep -Seconds 4
    $running = @($watchers | Where-Object { !$_.HasExited })
    if ($running.Count -ne 1) { throw ('Multiple startup requests left ' + $running.Count + ' active watchers instead of one') }
    $game = Start-Process $gameExe -ArgumentList '/k' -WindowStyle Hidden -PassThru
    $until = [DateTime]::UtcNow.AddSeconds(10)
    do {
        Start-Sleep -Milliseconds 250
        $trackers = @(Get-Process infst-singleton-tracker -ErrorAction SilentlyContinue | Where-Object Path -EQ $trackerExe)
    } while ($trackers.Count -eq 0 -and [DateTime]::UtcNow -lt $until)
    Start-Sleep -Seconds 3
    $trackers = @(Get-Process infst-singleton-tracker -ErrorAction SilentlyContinue | Where-Object Path -EQ $trackerExe)
    if ($trackers.Count -ne 1) { throw 'Multiple startup requests did not produce exactly one tracker' }
    $starts = @(Get-Content -LiteralPath (Join-Path $root 'autostart.log') | Where-Object { $_ -like '*Started tracker*' })
    if ($starts.Count -ne 1) { throw 'Tracker was started more than once' }
    Write-Output 'PASS: multiple startup requests keep one watcher and start one tracker'
} finally {
    foreach ($watcher in $watchers) {
        if (!$watcher.HasExited) { Stop-Process -Id $watcher.Id -ErrorAction SilentlyContinue }
        $watcher.Dispose()
    }
    Get-Process infst-singleton-tracker -ErrorAction SilentlyContinue | Where-Object Path -EQ $trackerExe | Stop-Process
    if ($game -and !$game.HasExited) { Stop-Process -Id $game.Id -ErrorAction SilentlyContinue }
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
