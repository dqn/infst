param([string]$WatcherPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'autostart.ps1'))
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:TEMP ('infst-autostart-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory $root | Out-Null
$gameExe = Join-Path $root 'infst-test-game.exe'
$trackerExe = Join-Path $root 'infst-test-tracker.exe'
Copy-Item "$env:WINDIR\System32\cmd.exe" $gameExe
Copy-Item "$env:WINDIR\System32\cmd.exe" $trackerExe
$game = $null
try {
    if (!(Test-Path $WatcherPath)) { throw 'Watcher entry point is missing' }
    $common = @{ GameProcessName = 'infst-test-game'; TrackerProcessName = 'infst-test-tracker'; TrackerPath = $trackerExe; WorkingDirectory = $root; LogPath = (Join-Path $root 'autostart.log'); TrackerArguments = @("/k"); Once = $true }
    & $WatcherPath @common
    if (@(Get-Process infst-test-tracker -ErrorAction SilentlyContinue).Count -ne 0) { throw 'Started tracker without a game' }
    $game = Start-Process $gameExe -ArgumentList '/k' -WindowStyle Hidden -PassThru
    & $WatcherPath @common
    Start-Sleep -Milliseconds 500
    if (@(Get-Process infst-test-tracker -ErrorAction SilentlyContinue).Count -ne 1) { throw 'Did not start exactly one tracker for a running game' }
    & $WatcherPath @common
    if (@(Get-Process infst-test-tracker -ErrorAction SilentlyContinue).Count -ne 1) { throw 'Started a duplicate tracker' }
    Write-Output 'PASS: no game -> no launch; game -> one launch; existing tracker -> no duplicate'
} finally {
    if ($game -and !$game.HasExited) { Stop-Process -Id $game.Id -ErrorAction SilentlyContinue }
    Get-Process infst-test-tracker -ErrorAction SilentlyContinue | Where-Object Path -EQ $trackerExe | Stop-Process
    Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
}
