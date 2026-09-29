# Builds kl!ck and its installer: web pages, window, service and CLI, then klick-setup.exe
# with the program packed inside. Result: dist\klick-setup.exe
# Usage: powershell -ExecutionPolicy Bypass -File build.ps1 [-Debug]
# (ASCII only: Windows PowerShell 5.1 reads BOM-less scripts in the ANSI code page.)
param([switch]$Debug)

# Continue: cargo prints progress to stderr, and PowerShell 5.1 treats redirected stderr as errors.
$ErrorActionPreference = 'Continue'
$app = $PSScriptRoot
$cfg = if ($Debug) { 'debug' } else { 'release' }
# [string[]]: PowerShell unrolls a one-item array from 'if', and splatting a plain string passes it char by char.
[string[]]$flag = if ($Debug) { @() } else { '--release' }

function Step([string]$name, [scriptblock]$run) {
    Write-Host "== $name"
    & $run
    if ($LASTEXITCODE) { throw "$name failed (exit code $LASTEXITCODE)" }
}

Push-Location $app
try {
    # 0. mihomo core and the country database are not in git: official release, sha256 checked.
    if (-not ((Test-Path "$app\resources\core\mihomo.exe") -and (Test-Path "$app\resources\core\Country.mmdb"))) {
        Step 'core' { powershell -NoProfile -ExecutionPolicy Bypass -File "$app\tools\fetch-core.ps1" }
    }
    # 1. Pages of the window and the installer: ui\dist (both exes embed it).
    Step 'web pages' { Push-Location ui; try { npm run build } finally { Pop-Location } }
    # 2. Window, service, CLI. The installer packs klick.exe and klick-service.exe from target\<cfg>.
    Step 'kl!ck' { cargo build @flag -p klick-service -p klick-cli -p klick-ui }
    # 3. Installer with the program inside.
    Step 'installer' { cargo build @flag -p klick-setup }

    New-Item -ItemType Directory -Force "$app\dist" | Out-Null
    Copy-Item "$app\target\$cfg\klick-setup.exe" "$app\dist\klick-setup.exe" -Force
    $mb = [math]::Round((Get-Item "$app\dist\klick-setup.exe").Length / 1MB, 1)
    Write-Host "== done: dist\klick-setup.exe ($mb MB, $cfg)"
}
finally {
    Pop-Location
}
