# CI (GitHub Actions, Windows runner): the same checks as in Windows Sandbox, on the runner itself,
# which is a fresh VM too. Stages the build into C:\klick like sandbox\start.ps1, runs one scenario in
# Windows PowerShell 5.1 (as in the Sandbox), prints its report and fails when the report has failures.
#   pwsh -File sandbox\ci.ps1 -Scenario run|setup|ks-browser|real|servers -Build <folder> [-Old <kl!ck 0.3.0 NSIS setup>]
# <folder>: klick-service.exe, klick-cli.exe, klick.exe, klick-setup.exe and resources\ (with core\mihomo.exe).
# real, servers: the subscription link comes in KLICK_TEST_SUB.
param(
    [Parameter(Mandatory)][ValidateSet('run', 'setup', 'ks-browser', 'real', 'servers')][string]$Scenario,
    [Parameter(Mandatory)][string]$Build,
    [string]$Old
)

$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$root = 'C:\klick'
if (Test-Path $root) { Remove-Item $root -Recurse -Force }
New-Item -ItemType Directory -Force "$root\bin", "$root\results" | Out-Null
Copy-Item "$Build\klick-service.exe", "$Build\klick-cli.exe", "$Build\klick.exe" "$root\bin"
Copy-Item "$Build\klick-setup.exe" "$root\klick-setup.exe"
Copy-Item "$Build\resources" "$root\resources" -Recurse
# Windows PowerShell 5.1 reads a script as UTF-8 only when it has a BOM.
foreach ($s in 'run.ps1', 'setup-test.ps1', 'ks-browser.ps1', 'real-test.ps1', 'servers-test.ps1', 'ui.ps1') {
    $text = [IO.File]::ReadAllText("$PSScriptRoot\$s", [Text.Encoding]::UTF8)
    [IO.File]::WriteAllText("$root\$s", $text, (New-Object Text.UTF8Encoding $true))
}
if ($Scenario -eq 'ks-browser') {
    Copy-Item "$PSScriptRoot\ks-browser.mjs" "$root\ks-browser.mjs"
    New-Item -ItemType Directory -Force "$root\node" | Out-Null
    Copy-Item (Get-Command node.exe).Source "$root\node\node.exe"
}
if ($Old) { Copy-Item $Old "$root\old-nsis.exe" }

# Сценарий, его отчёты, строка итога и предел времени в минутах.
$plan = @{
    'run'        = @('run.ps1', @('report.txt'), 'ИТОГ: провалов (\d+)', 15)
    'setup'      = @('setup-test.ps1', @('setup-report.txt', 'report.txt'), 'ИТОГ УСТАНОВЩИКА: провалов (\d+)', 30)
    'ks-browser' = @('ks-browser.ps1', @('ks-browser.txt'), 'ИТОГ: шагов \d+, провалов (\d+)', 25)
    'real'       = @('real-test.ps1', @('real-report.txt'), 'ИТОГ: провалов (\d+)', 25)
    'servers'    = @('servers-test.ps1', @('servers-report.txt'), 'ИТОГ: провалов (\d+)', 35)
}[$Scenario]
$entry, $reports, $summary, $minutes = $plan

Write-Host "== $Scenario ($entry) on $((Get-CimInstance Win32_OperatingSystem).Caption) $([Environment]::OSVersion.Version)"
$ErrorActionPreference = 'Continue'
# Windows PowerShell 5.1 из pwsh наследует PSModulePath с модулями PowerShell 7 и теряет часть команд
# (Get-FileHash и др.); без переменной 5.1 берёт свои пути, как в Песочнице.
Remove-Item Env:PSModulePath -ErrorAction SilentlyContinue
$ps = Start-Process "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -PassThru -NoNewWindow `
    -ArgumentList '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "$root\$entry", '-NoShutdown' `
    -RedirectStandardOutput "$root\results\run.out" -RedirectStandardError "$root\results\run.err"
$timedOut = -not $ps.WaitForExit($minutes * 60 * 1000)
if ($timedOut) {
    Write-Host "== $Scenario не закончился за $minutes мин — останавливаю, ниже отчёт на этот момент"
    & taskkill.exe /PID $ps.Id /T /F | Out-Null
}

foreach ($r in $reports) {
    $f = "$root\results\$r"
    if (Test-Path $f) { Write-Host "===== $r"; Get-Content $f -Encoding UTF8 | ForEach-Object { Write-Host $_ } }
}
$main = "$root\results\$($reports[0])"
$sum = if (Test-Path $main) { Get-Content $main -Encoding UTF8 | Select-String $summary | Select-Object -Last 1 } else { $null }
$fails = if ($sum) { [int]$sum.Matches[0].Groups[1].Value } else { -1 }
if ($fails -ne 0) {
    Write-Host '===== script output'
    Get-Content "$root\results\run.out", "$root\results\run.err" -ErrorAction SilentlyContinue | Select-Object -Last 40 | ForEach-Object { Write-Host $_ }
    $log = if (Test-Path "$root\results\service.log") { "$root\results\service.log" } else { 'C:\ProgramData\klick\logs\service.log' }
    Write-Host "===== service log ($log)"
    Get-Content $log -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -Last 120 | ForEach-Object { Write-Host $_ }
}
if ($timedOut) { exit 1 }
if ($fails -lt 0) { Write-Host "== no summary in $($reports[0])"; exit 1 }
Write-Host "== $Scenario`: failures $fails"
exit ([int]($fails -gt 0))
