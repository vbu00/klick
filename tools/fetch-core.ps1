# Скачивает ядро и GeoIP-базу, с которыми kl!ck собирается и проверен:
# официальный релиз mihomo (MetaCubeX) и country.mmdb из meta-rules-dat,
# привязанный к коммиту. Обе контрольные суммы сверяются — подменённый или
# битый файл в сборку не попадёт.
#
#   npm run core          (или powershell -File tools/fetch-core.ps1)
#
# Обновить ядро: поменять $MihomoVersion и $MihomoSha256 (sha256 — поле
# digest у файла в https://api.github.com/repos/MetaCubeX/mihomo/releases).

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$MihomoVersion = 'v1.19.31'
$MihomoSha256  = '93d14e9a13b49b2f2d256202d02cc8d14a7c4695edf084cae0f941986bc9c218'
$GeoCommit     = '6b01e65c89cdeb684c00c70bb0b414091b22f124'
$GeoSha256     = '21606dfffd4ec39542ec40782bebd37b71310471816e1338b6f6430190cbc85d'

$bin = Join-Path $PSScriptRoot '..\src-tauri\bin'
New-Item -ItemType Directory -Force $bin | Out-Null
$tmp = Join-Path ([IO.Path]::GetTempPath()) ('klick-core-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force $tmp | Out-Null

function Get-Checked($url, $file, $sha) {
    Write-Host "↓ $url"
    Invoke-WebRequest -UseBasicParsing -Uri $url -OutFile $file
    $got = (Get-FileHash -Algorithm SHA256 $file).Hash.ToLower()
    if ($got -ne $sha) { throw "sha256 не совпал для $url`n  ждали $sha`n  пришло $got" }
    Write-Host "  sha256 совпал"
}

try {
    $zipName = "mihomo-windows-amd64-compatible-$MihomoVersion.zip"
    $zip = Join-Path $tmp $zipName
    Get-Checked "https://github.com/MetaCubeX/mihomo/releases/download/$MihomoVersion/$zipName" $zip $MihomoSha256
    Expand-Archive -Force $zip $tmp
    Move-Item -Force (Join-Path $tmp 'mihomo-windows-amd64-compatible.exe') (Join-Path $bin 'mihomo.exe')

    Get-Checked "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/$GeoCommit/country.mmdb" (Join-Path $bin 'Country.mmdb') $GeoSha256

    & (Join-Path $bin 'mihomo.exe') -v
    Write-Host "Готово: $((Resolve-Path $bin).Path)"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
