param([string]$RegistrationPath = (Join-Path (Split-Path $PSScriptRoot -Parent) 'register-autostart.ps1'))
$ErrorActionPreference = 'Stop'
$root = Join-Path $env:TEMP ("infst 日本語 registration [test] & O'Brien " + [guid]::NewGuid().ToString('N'))
$install = Join-Path $root 'installed'
$tracker = Join-Path $root 'infst-registration-tracker.exe'
$taskName = 'Infst Registration Test ' + [guid]::NewGuid().ToString('N')
New-Item -ItemType Directory -Path $root | Out-Null
Copy-Item -LiteralPath "$env:WINDIR\System32\cmd.exe" -Destination $tracker
try {
    $options = @{ TrackerPath = $tracker; WorkingDirectory = $root; InstallDirectory = $install; TaskName = $taskName }
    foreach ($attempt in 1..2) {
        & $RegistrationPath @options | Out-Null
        Start-Sleep -Seconds 3
        $task = Get-ScheduledTask -TaskName $taskName
        if ($task.State -ne 'Running') { throw 'Registered watcher did not stay running in the desktop session' }
        if ($task.Principal.LogonType -ne 'Interactive') { throw 'Watcher is not registered for the interactive desktop' }
        if ($task.Settings.MultipleInstances -ne 'IgnoreNew') { throw 'Repeated task starts can create concurrent watchers' }
        if ($task.Settings.ExecutionTimeLimit -ne 'PT0S') { throw 'Watcher has a finite execution time limit' }
        if ($task.Triggers.Count -ne 1 -or $task.Triggers[0].CimClass.CimClassName -ne 'MSFT_TaskLogonTrigger') { throw 'Watcher is not started at user logon' }
        if ($task.Actions[0].WorkingDirectory -ne $root) { throw 'Registered working directory changed' }
        $installed = Join-Path $install 'autostart.ps1'
        if (!(Test-Path -LiteralPath $installed -PathType Leaf)) { throw 'Watcher was not installed' }
        if (@(Get-ScheduledTask | Where-Object TaskName -EQ $taskName).Count -ne 1) { throw 'Re-registration created duplicate tasks' }
    }
    Write-Output 'PASS: registration installs a running logon watcher; paths with spaces, apostrophes and brackets survive re-registration'
} finally {
    if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
        Stop-ScheduledTask -TaskName $taskName
        Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
    }
    Get-Process infst-registration-tracker -ErrorAction SilentlyContinue | Where-Object Path -EQ $tracker | Stop-Process
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
