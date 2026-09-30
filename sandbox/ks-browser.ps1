# Kill Switch и браузер: воспроизведение жалобы «Chrome с Kill Switch в режиме «Системный прокси»
# пару раз работает, потом перестаёт». Внутри Песочницы Edge — тот же движок и тот же разбор
# системного прокси, что у Chrome. kl!ck ставится установщиком, окно kl!ck работает (оно и ставит
# системный прокси), Edge живёт всё время, как браузер у человека; VPN включается и выключается много раз.

# -NoShutdown: не выключать компьютер в конце (CI: раннер GitHub, sandbox\ci.ps1).
param([switch]$NoShutdown)

$ErrorActionPreference = 'Continue'
$root = 'C:\klick'
$out = Join-Path $root 'results'
$inst = 'C:\Program Files\klick'
New-Item -ItemType Directory -Force $out | Out-Null
$report = Join-Path $out 'ks-browser.txt'
Set-Content -Path $report -Value '' -Encoding UTF8
function Log([string]$m) {
    $line = "[{0:HH:mm:ss}] {1}" -f (Get-Date), $m
    for ($i = 0; $i -lt 20; $i++) { try { Add-Content -Path $report -Value $line -Encoding UTF8 -ErrorAction Stop; return } catch { Start-Sleep -Milliseconds 250 } }
}
function KlickCli { (& "$inst\klick-cli.exe" --prod @args 2>&1 | Out-String).Trim() }

try {
    # kl!ck — установщиком, как у человека
    $p = Start-Process "$root\klick-setup.exe" -ArgumentList '--silent', '--no-autostart' -Wait -PassThru -WindowStyle Hidden
    Log "установка: код $($p.ExitCode)"
    Copy-Item "$root\bin\klick-cli.exe" $inst -Force

    # Тестовый «VPN-сервер»: второй mihomo с socks5 (как в run.ps1)
    $route = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric | Select-Object -First 1
    $nic = if ($route) { Get-NetAdapter -InterfaceIndex $route.ifIndex } else { Get-NetAdapter | Where-Object { $_.Status -eq 'Up' -and $_.Name -ne 'klick' } | Select-Object -First 1 }
    $ip = (Get-NetIPAddress -InterfaceIndex $nic.ifIndex -AddressFamily IPv4 | Select-Object -First 1).IPAddress
    $srv = 'C:\srv'
    New-Item -ItemType Directory -Force $srv | Out-Null
    $srvCfg = [ordered]@{
        'mode' = 'rule'; 'log-level' = 'warning'; 'interface-name' = $nic.Name
        'listeners' = @(@{ name = 'srv'; type = 'socks'; port = 1080; listen = '0.0.0.0'; udp = $true })
        'dns' = @{ enable = $true; nameserver = @('https://1.1.1.1/dns-query') }
        'rules' = @('MATCH,DIRECT')
    } | ConvertTo-Json -Depth 5
    Set-Content "$srv\config.yaml" $srvCfg -Encoding ASCII
    $srvProc = Start-Process "$inst\resources\core\mihomo.exe" -ArgumentList '-d', $srv, '-f', "$srv\config.yaml" -RedirectStandardOutput "$out\srv.log" -RedirectStandardError "$out\srv.err" -WindowStyle Hidden -PassThru
    Start-Sleep 2
    Set-Content "$root\local.yaml" "proxies: [{name: nic, type: socks5, server: $ip, port: 1080, udp: true}]" -Encoding ASCII
    Log ("import: " + (KlickCli import "$root\local.yaml"))
    KlickCli routing all | Out-Null

    # Edge — в Kill Switch; режим «Системный прокси»
    $edgeDir = 'C:\Program Files (x86)\Microsoft\Edge\Application'
    Log ("ks add: " + ((KlickCli ks add $edgeDir) -replace '\s+', ' '))
    KlickCli mode proxy | Out-Null

    # Окно kl!ck — оно ставит системный прокси
    $win = Start-Process "$inst\klick.exe" -PassThru
    Start-Sleep 5
    Log "окно kl!ck: pid $($win.Id), работает: $(-not $win.HasExited)"

    # Сценарий в браузере — node + протокол отладки Edge
    # Не дольше 20 минут: зависший браузер не должен держать проверку бесконечно.
    $node = Start-Process "$root\node\node.exe" -ArgumentList "`"$root\ks-browser.mjs`"" -RedirectStandardOutput "$out\ks-node.out" -RedirectStandardError "$out\ks-node.err" -WindowStyle Hidden -PassThru
    if (-not $node.WaitForExit(20 * 60 * 1000)) {
        Log 'ОШИБКА СЦЕНАРИЯ: браузерная часть не закончилась за 20 минут, останавливаю'
        & taskkill.exe /PID $node.Id /T /F | Out-Null
    }
    Get-Content "$out\ks-node.out", "$out\ks-node.err" -Encoding UTF8 -ErrorAction SilentlyContinue | ForEach-Object { Log "$_" }
}
catch {
    Log "ОШИБКА СЦЕНАРИЯ: $($_.Exception.Message)"
}
finally {
    # Окно kl!ck и тестовый сервер наследуют вывод этого сценария: живые, они держат его открытым.
    foreach ($p in @($win, $srvProc)) { if ($p) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue } }
    Copy-Item 'C:\ProgramData\klick\logs\service.log' "$out\service.log" -ErrorAction SilentlyContinue
    Set-Content "$out\script-done.txt" 'done'
    if (-not $NoShutdown) {
        Start-Sleep 2
        shutdown.exe /s /t 0
    }
}
