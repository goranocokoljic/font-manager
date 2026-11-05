#!/usr/bin/env pwsh
# install-keeper.ps1 — run the go-harness keeper at every logon, in a terminal window.
#
#   ./scripts/harness/install-keeper.ps1             # register the task "go-harness keeper" and start it now
#   ./scripts/harness/install-keeper.ps1 -Uninstall  # remove the task (a running keeper keeps running)
#   ./scripts/harness/install-keeper.ps1 -Model fable
#
# The task runs only while you are logged on (the lanes it starts need a desktop
# for their terminal tabs) and does not stop on battery or after a time limit.
# The keeper itself is idempotent: a second start exits at once (lane lock).

[CmdletBinding()]
param(
    [switch]$Uninstall,
    [ValidateSet('sonnet', 'opus', 'fable')]
    [string]$Model = 'opus'
)

$ErrorActionPreference = 'Stop'
$name = 'go-harness keeper'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$script = Join-Path $repo 'go-harness.ps1'
$pwsh = (Get-Command pwsh).Source

if ($Uninstall) {
    if (Get-ScheduledTask -TaskName $name -ErrorAction SilentlyContinue) {
        Unregister-ScheduledTask -TaskName $name -Confirm:$false
        Write-Host "removed the scheduled task '$name'. A running keeper keeps running; close its window to stop it."
    } else { Write-Host "no scheduled task '$name'" }
    return
}

if (-not (Test-Path $script)) { throw "go-harness.ps1 not found at $script" }

# wt.exe (Windows Terminal) gives the keeper a tab and lets it open tabs for the lanes.
$wt = Get-Command wt.exe -ErrorAction SilentlyContinue
$laneArgs = "-NoProfile -NoExit -File `"$script`" -Lane keeper -Model $Model"
$action = if ($wt) {
    New-ScheduledTaskAction -Execute $wt.Source -Argument "-w 0 new-tab --title `"go-harness keeper`" `"$pwsh`" $laneArgs" -WorkingDirectory $repo
} else {
    New-ScheduledTaskAction -Execute $pwsh -Argument $laneArgs -WorkingDirectory $repo
}
$trigger = New-ScheduledTaskTrigger -AtLogOn -User $env:USERNAME
$trigger.Delay = 'PT1M'   # let the network and the terminal come up first
$settings = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
    -ExecutionTimeLimit ([TimeSpan]::Zero) -StartWhenAvailable -MultipleInstances IgnoreNew
$principal = New-ScheduledTaskPrincipal -UserId $env:USERNAME -LogonType Interactive -RunLevel Limited

Register-ScheduledTask -TaskName $name -Action $action -Trigger $trigger -Settings $settings -Principal $principal -Force | Out-Null
Write-Host "registered '$name': runs at your logon (1 min delay) in $(if ($wt) { 'a Windows Terminal tab' } else { 'a PowerShell window' })."
Write-Host "    change: Task Scheduler > Task Scheduler Library > '$name'"
Write-Host "    remove: ./scripts/harness/install-keeper.ps1 -Uninstall"

$lock = Join-Path $repo '.harness\state\keeper.lock'
$running = $false
if (Test-Path $lock) {
    try { $pid0 = (Get-Content $lock -Raw | ConvertFrom-Json).pid; $running = [bool](Get-Process -Id $pid0 -ErrorAction SilentlyContinue) } catch { }
}
if ($running) { Write-Host "    a keeper already runs (PID $pid0); not started again." }
else { Start-ScheduledTask -TaskName $name; Write-Host "    started the keeper now." }
