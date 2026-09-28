# Runs the kl!ck check inside Windows Sandbox: stages the build and the scripts, opens the Sandbox, waits for the report.
# Usage: powershell -File sandbox\start.ps1 [-Release] [-Setup] [-NoWait]
#   -Setup: check the installer instead (setup-test.ps1): an old kl!ck 0.2 is staged from this PC's
#           Program Files (program files only, no user data), then klick-setup.exe installs over it,
#           run.ps1 checks the installed copy, and the installer updates and uninstalls it.
# (ASCII only: Windows PowerShell 5.1 reads BOM-less scripts in the ANSI code page.)
#   -Script <name>: run sandbox\<name>.ps1 instead (kl!ck is installed there by klick-setup.exe;
#           node.exe from this PC and sandbox\<name>.mjs are staged too). Report: results\<name>.txt
param([switch]$Release, [switch]$Setup, [string]$Script, [switch]$NoWait)

$app = Split-Path $PSScriptRoot -Parent
$cfg = if ($Release) { 'release' } else { 'debug' }
$stage = Join-Path $env:TEMP 'klick-sbx'
# .NET instead of Remove-Item: the provider cannot resolve the 8.3 short TEMP path (C:\Users\ABCD~1).
if (Test-Path $stage) { [IO.Directory]::Delete($stage, $true) }
New-Item -ItemType Directory -Force "$stage\bin", "$stage\results" | Out-Null
Copy-Item "$app\target\$cfg\klick-service.exe", "$app\target\$cfg\klick-cli.exe", "$app\target\$cfg\klick.exe" "$stage\bin"
Copy-Item "$app\resources" "$stage\resources" -Recurse
# PowerShell 5.1 inside the Sandbox reads a script as UTF-8 only when it has a BOM.
$scripts = @('run.ps1', 'setup-test.ps1')
if ($Script) { $scripts += "$Script.ps1" }
foreach ($s in $scripts) {
    $text = [IO.File]::ReadAllText("$PSScriptRoot\$s", [Text.Encoding]::UTF8)
    [IO.File]::WriteAllText("$stage\$s", $text, (New-Object Text.UTF8Encoding $true))
}

# $entry, not $script: PowerShell names are case-insensitive, and $Script is the parameter.
$entry = 'run.ps1'
$doneFile = 'done.txt'
$minutes = 9
if ($Setup) {
    Copy-Item "$app\target\$cfg\klick-setup.exe" "$stage\klick-setup.exe"
    $old = Join-Path $env:ProgramW6432 'kl!ck'
    New-Item -ItemType Directory -Force "$stage\old" | Out-Null
    foreach ($f in 'klick.exe', 'uninstall.exe', 'LICENSE.txt', 'THIRD_PARTY_NOTICES.md') {
        if (Test-Path -LiteralPath "$old\$f") { Copy-Item -LiteralPath "$old\$f" "$stage\old\$f" }
    }
    $entry = 'setup-test.ps1'
    $doneFile = 'setup-done.txt'
    $minutes = 18
}
if ($Script) {
    Copy-Item "$app\target\$cfg\klick-setup.exe" "$stage\klick-setup.exe"
    New-Item -ItemType Directory -Force "$stage\node" | Out-Null
    Copy-Item (Get-Command node.exe).Source "$stage\node\node.exe"
    if (Test-Path "$PSScriptRoot\$Script.mjs") { Copy-Item "$PSScriptRoot\$Script.mjs" "$stage\$Script.mjs" }
    $entry = "$Script.ps1"
    $doneFile = 'script-done.txt'
    $minutes = 20
}

$wsb = @"
<Configuration>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>$stage</HostFolder>
      <SandboxFolder>C:\klick</SandboxFolder>
      <ReadOnly>false</ReadOnly>
    </MappedFolder>
  </MappedFolders>
  <LogonCommand>
    <Command>cmd.exe /c "echo logon %TIME% &gt; C:\klick\results\logon.txt &amp; powershell.exe -NoProfile -ExecutionPolicy Bypass -File C:\klick\$entry &gt; C:\klick\results\run.out 2&gt;&amp;1"</Command>
  </LogonCommand>
  <MemoryInMB>4096</MemoryInMB>
</Configuration>
"@
Set-Content -LiteralPath "$stage\klick-test.wsb" -Value $wsb -Encoding UTF8
Start-Process "$env:WINDIR\System32\WindowsSandbox.exe" -ArgumentList "`"$stage\klick-test.wsb`""
"Sandbox started, stage: $stage"
if ($NoWait) { return }

$started = Get-Date
$deadline = $started.AddMinutes($minutes)
# Only check that the file exists: reading files in the shared folder while the Sandbox writes them blocks its writes.
while (-not (Test-Path "$stage\results\$doneFile") -and (Get-Date) -lt $deadline) { Start-Sleep 5 }
$reports = @('setup-report.txt', 'report.txt')
if ($Script) { $reports = @("$Script.txt") }
foreach ($r in $reports) {
    if (Test-Path "$stage\results\$r") { "===== $r"; [IO.File]::ReadAllText("$stage\results\$r", [Text.Encoding]::UTF8) }
}
if (-not (Test-Path "$stage\results\$doneFile")) { 'no report (timeout)'; return }
# The Sandbox shuts itself down, but its window sometimes stays open: close ours once the machine is off.
Start-Sleep 20
Get-Process WindowsSandboxRemoteSession -ErrorAction SilentlyContinue | Where-Object { $_.StartTime -ge $started.AddMinutes(-1) } | ForEach-Object { [void]$_.CloseMainWindow() }
