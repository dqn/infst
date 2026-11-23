param(
    [Parameter(Mandatory)][string]$TrackerPath,
    [string]$WorkingDirectory,
    [string]$InstallDirectory = (Join-Path $env:LOCALAPPDATA 'infst'),
    [string]$TaskName = 'Infst Auto Start'
)
$ErrorActionPreference = 'Stop'
$source = Join-Path $PSScriptRoot 'autostart.ps1'
if (!(Test-Path -LiteralPath $source -PathType Leaf)) { throw "Watcher not found: $source" }
if (!(Test-Path -LiteralPath $TrackerPath -PathType Leaf)) { throw "Tracker not found: $TrackerPath" }
$TrackerPath = (Resolve-Path -LiteralPath $TrackerPath).ProviderPath
if (!$WorkingDirectory) { $WorkingDirectory = Split-Path -Parent $TrackerPath }
if (!(Test-Path -LiteralPath $WorkingDirectory -PathType Container)) { throw "Working directory not found: $WorkingDirectory" }
$WorkingDirectory = (Resolve-Path -LiteralPath $WorkingDirectory).ProviderPath
New-Item -ItemType Directory -Path $InstallDirectory -Force | Out-Null
$InstallDirectory = (Resolve-Path -LiteralPath $InstallDirectory).ProviderPath
$watcher = Join-Path $InstallDirectory 'autostart.ps1'

# Encode PowerShell literals so paths are independent of command-line quoting.
$command = "& '{0}' -TrackerPath '{1}' -WorkingDirectory '{2}'" -f $watcher.Replace("'", "''"), $TrackerPath.Replace("'", "''"), $WorkingDirectory.Replace("'", "''")
$encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($command))
$powershell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
$action = New-ScheduledTaskAction -Execute $powershell -Argument "-NoProfile -NonInteractive -WindowStyle Hidden -ExecutionPolicy Bypass -EncodedCommand $encoded" -WorkingDirectory $WorkingDirectory
$userId = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$principal = New-ScheduledTaskPrincipal -UserId $userId -LogonType Interactive
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $userId
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries -ExecutionTimeLimit ([TimeSpan]::Zero) -MultipleInstances IgnoreNew -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -StartWhenAvailable

$existing = Get-ScheduledTask -TaskName $TaskName -ErrorAction SilentlyContinue
if ($existing) {
    Stop-ScheduledTask -TaskName $TaskName
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while ((Get-ScheduledTask -TaskName $TaskName).State -eq 'Running') {
        if ([DateTime]::UtcNow -ge $deadline) { throw "Task did not stop: $TaskName" }
        Start-Sleep -Milliseconds 100
    }
}
Copy-Item -LiteralPath $source -Destination $watcher -Force
Register-ScheduledTask -TaskName $TaskName -Action $action -Trigger $trigger -Principal $principal -Settings $settings -Description 'Start infst without activating its console when INFINITAS starts; keep one watcher per desktop session.' -Force | Out-Null
Start-ScheduledTask -TaskName $TaskName
Write-Output "Registered and started '$TaskName' for $TrackerPath"
