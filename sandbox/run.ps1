# Проверка kl!ck внутри Песочницы Windows. Запускается сама при входе (LogonCommand).
# Роль VPN-сервера играет второй mihomo внутри Песочницы (socks5), поэтому ключи настоящих серверов не нужны.
# -Preinstalled: kl!ck уже поставил установщик (setup-test.ps1), здесь только проверки.
# -KeepInstalled: не удалять службу в конце и не выключать Песочницу — дальше проверяет установщик.
# -NoShutdown: не выключать компьютер в конце (CI: раннер GitHub, sandbox\ci.ps1).
param([switch]$Preinstalled, [switch]$KeepInstalled, [switch]$NoShutdown)

$ErrorActionPreference = 'Continue'
$root = 'C:\klick'
$out = Join-Path $root 'results'
$inst = 'C:\Program Files\klick'
New-Item -ItemType Directory -Force $out | Out-Null
$report = Join-Path $out 'report.txt'
Set-Content -Path $report -Value '' -Encoding UTF8
$script:fails = 0
$guardWatch = $null

function Log([string]$m) { Add-Content -Path $report -Value ("[{0:HH:mm:ss}] {1}" -f (Get-Date), $m) -Encoding UTF8 }
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if (-not $ok) { $script:fails++ }
    $mark = if ($ok) { 'OK  ' } else { 'FAIL' }
    $tail = if ($detail) { " - $detail" } else { '' }
    Log "$mark $name$tail"
}
function KlickCli { (& "$inst\klick-cli.exe" --prod @args 2>&1 | Out-String).Trim() }
# Окно подключилось к службе: служба пишет об этом строку в журнал. Сколько таких строк уже есть.
function WindowLinks { @(Get-Content 'C:\ProgramData\klick\logs\service.log' -Encoding UTF8 -ErrorAction SilentlyContinue | Select-String 'подключилось окно').Count }
# Дождаться новой такой строки: WebView2 на новой машине стартует долго, фиксированной паузы мало.
function WaitWindow([int]$before, [int]$seconds = 45) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if ((WindowLinks) -gt $before) { return $true }
        Start-Sleep -Milliseconds 300
    }
    $false
}
# ConvertFrom-Json в PowerShell 5.1 отдаёт JSON-массив одним объектом; ForEach-Object раскладывает его на элементы.
function KlickJson { $t = & "$inst\klick-cli.exe" --prod @args 2>$null | Out-String; try { $t | ConvertFrom-Json | ForEach-Object { $_ } } catch { $null } }
function WaitVpn([string]$want, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    do { $s = KlickJson status; if ($s.vpn -eq $want) { return $s }; Start-Sleep 1 } while ((Get-Date) -lt $end)
    return $s
}
function Http([string]$exe, [string]$url = 'https://www.gstatic.com/generate_204', [string[]]$extra = @()) {
    $code = & $exe -s -o NUL -w '%{http_code}' --max-time 12 @extra $url 2>$null
    if ($code) { "$code".Trim() } else { '000' }
}
# В образе Песочницы бывает без WebView2 — без него окно kl!ck не открывается («Could not find the
# WebView2 Runtime»), и проверки окна ложно проваливаются. Ставим официальный загрузчик Microsoft.
function WebView2Installed {
    foreach ($k in 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
                   'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}') {
        $pv = (Get-ItemProperty $k -ErrorAction SilentlyContinue).pv
        if ($pv -and $pv -ne '0.0.0.0') { return $true }
    }
    return $false
}
function EnsureWebView2 {
    if (WebView2Installed) { Log 'WebView2 уже есть'; return }
    $t0 = Get-Date
    # Полный установщик, если start.ps1 его положил (sandbox\cache): загрузчик в Песочнице качает
    # от полутора минут до четверти часа.
    $f = Join-Path $root 'webview2.exe'
    if (-not (Test-Path $f)) {
        $f = Join-Path $env:TEMP 'MicrosoftEdgeWebview2Setup.exe'
        try { Invoke-WebRequest 'https://go.microsoft.com/fwlink/p/?LinkId=2124703' -OutFile $f -UseBasicParsing } catch { Log "WebView2 не скачался: $($_.Exception.Message)"; return }
    }
    $sig = Get-AuthenticodeSignature $f
    if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation') { Log 'WebView2: подпись не Microsoft — не запускаю'; return }
    # WaitForExit, а не -Wait: -Wait ждёт и фоновые процессы Edge Update, которые остаются жить.
    [void](Start-Process $f -ArgumentList '/silent', '/install' -PassThru).WaitForExit(600000)
    Log ("WebView2 поставлен за {0:N0} с: {1}" -f ((Get-Date) - $t0).TotalSeconds, (WebView2Installed))
}
function SrvLines { @(Get-Content "$out\srv.log" -ErrorAction SilentlyContinue) }
function ProxyReg { Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' }
function CoreProc { Get-CimInstance Win32_Process -Filter "Name='mihomo.exe'" | Where-Object { $_.CommandLine -like '*ProgramData*' } }
function CorePid {
    $p = @(CoreProc)
    if ($p.Count -eq 1) { return [int]$p[0].ProcessId }
    return 0
}
function WatchText([string]$path) {
    try { if (Test-Path $path) { return [IO.File]::ReadAllText($path) } } catch {}
    return ''
}
function GuardProbeCount {
    $lines = @(SrvLines | Where-Object { $_ -match 'gstatic|cp\.cloudflare\.com' })
    return $lines.Count
}
function WaitGuardProbe([int]$seconds) {
    $before = GuardProbeCount
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if ((GuardProbeCount) -gt $before) { return $true }
        Start-Sleep -Milliseconds 250
    }
    return $false
}
function TestSrvListener {
    $client = New-Object System.Net.Sockets.TcpClient
    try {
        $pending = $client.BeginConnect('127.0.0.1', 1080, $null, $null)
        if ($pending.AsyncWaitHandle.WaitOne(250)) {
            $client.EndConnect($pending)
            return $true
        }
    } catch {} finally { $client.Close() }
    return $false
}
function WaitSrv([int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        if (TestSrvListener) { return $true }
        Start-Sleep -Milliseconds 250
    }
    return $false
}
function FindConn([string]$processName, [string]$route) {
    $rows = @(KlickJson conns)
    return $rows | Where-Object {
        $_.route -eq $route -and $_.host -match 'speed\.cloudflare\.com' -and
        ($_.process -eq $processName -or $_.process_path -like "*\$processName")
    } | Select-Object -First 1
}
function WaitConn([string]$processName, [string]$route, [int]$seconds = 15) {
    $end = (Get-Date).AddSeconds($seconds)
    do {
        $row = FindConn $processName $route
        if ($row) { return $row }
        Start-Sleep -Milliseconds 500
    } while ((Get-Date) -lt $end)
    return $null
}
function StartTestDownload([string]$exe, [string]$file, [string]$err, [string]$rate = '40k') {
    if (Test-Path $file) { Remove-Item -LiteralPath $file -Force }
    Start-Process -FilePath $exe -ArgumentList @('-sS', '--limit-rate', $rate, '--max-time', '120', '-o', $file, 'https://speed.cloudflare.com/__down?bytes=20000000') -RedirectStandardError $err -PassThru -WindowStyle Hidden
}

try {
    $admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    Log ("Песочница: Windows {0}, права администратора: {1}" -f [Environment]::OSVersion.Version, $admin)
    EnsureWebView2

    # 1. Установка службы
    if ($Preinstalled) {
        # Установщик кладёт только окно и службу; клиент командной строки нужен проверкам.
        Copy-Item "$root\bin\klick-cli.exe" $inst -Force
    } else {
        New-Item -ItemType Directory -Force $inst | Out-Null
        Copy-Item "$root\bin\klick-service.exe", "$root\bin\klick-cli.exe", "$root\bin\klick.exe" $inst -Force
        Copy-Item "$root\resources" "$inst\resources" -Recurse -Force
        Log ("install: " + ((& "$inst\klick-service.exe" install 2>&1 | Out-String).Trim()))
    }
    Start-Sleep 2
    $svc = Get-Service klick -ErrorAction SilentlyContinue
    Check 'служба установлена и работает' ($svc -and $svc.Status -eq 'Running')
    $st = KlickJson status
    Check 'служба отвечает по каналу управления' ($null -ne $st) ("vpn=" + $st.vpn)
    $who = (Get-Acl 'C:\ProgramData\klick').Access | ForEach-Object { $_.IdentityReference.Value } | Sort-Object -Unique
    Check 'папка данных закрыта от обычных пользователей' (-not ($who -match 'Users|Пользователи|Authenticated|Everyone|Все')) ($who -join ', ')

    # 2. Тестовый «VPN-сервер»: второй mihomo с socks5
    # Карта с маршрутом по умолчанию: на раннере CI рядом бывают виртуальные (vEthernet у Docker).
    $route = Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Sort-Object RouteMetric | Select-Object -First 1
    $nic = if ($route) { Get-NetAdapter -InterfaceIndex $route.ifIndex } else { Get-NetAdapter | Where-Object { $_.Status -eq 'Up' -and $_.Name -ne 'klick' } | Select-Object -First 1 }
    $ip = (Get-NetIPAddress -InterfaceIndex $nic.ifIndex -AddressFamily IPv4 | Select-Object -First 1).IPAddress
    Log "сетевая карта: $($nic.Name), адрес $ip"
    $srv = 'C:\srv'
    New-Item -ItemType Directory -Force $srv | Out-Null
    $srvCfg = [ordered]@{
        'mode' = 'rule'; 'log-level' = 'info'; 'interface-name' = $nic.Name
        'listeners' = @(@{ name = 'srv'; type = 'socks'; port = 1080; listen = '0.0.0.0'; udp = $true })
        'dns' = @{ enable = $true; nameserver = @('https://1.1.1.1/dns-query') }
        'rules' = @('MATCH,DIRECT')
    } | ConvertTo-Json -Depth 5
    Set-Content "$srv\config.yaml" $srvCfg -Encoding ASCII
    function StartSrv { Start-Process "$inst\resources\core\mihomo.exe" -ArgumentList '-d', $srv, '-f', "$srv\config.yaml" -RedirectStandardOutput "$out\srv.log" -RedirectStandardError "$out\srv.err" -WindowStyle Hidden -PassThru }
    $srvProc = StartSrv
    Start-Sleep 2

    # 3. Подключение из файла: сервер по адресу карты и по 127.0.0.1
    Set-Content "$root\local.yaml" "proxies: [{name: nic, type: socks5, server: $ip, port: 1080, udp: true}, {name: loopback, type: socks5, server: 127.0.0.1, port: 1080, udp: true}]" -Encoding ASCII
    Log ("import: " + (KlickCli import "$root\local.yaml"))
    $servers = @(KlickJson servers)
    Check 'серверы читаются при выключенном VPN' ($servers.Count -eq 2) (($servers | ForEach-Object { $_.name }) -join ', ')
    $lat = @(KlickJson latency)
    Log ("задержка: " + (($lat | ForEach-Object { "$($_.name)=$($_.delay)" }) -join ', '))
    Check 'задержка меряется при выключенном VPN' (@($lat | Where-Object { $_.delay -gt 0 }).Count -gt 0)

    $base = Http 'curl.exe'
    Check 'интернет напрямую до подключения' ($base -eq '204') "код $base"

    # 4. VPN (TUN), «Всё через VPN»: пробуем оба сервера
    KlickCli routing all | Out-Null
    $working = $null
    foreach ($name in @('nic', 'loopback')) {
        KlickCli server $name | Out-Null
        if (-not $working) { KlickCli connect | Out-Null; Start-Sleep 3 }
        $n0 = (SrvLines).Count
        $code = Http 'curl.exe'
        Start-Sleep 1
        $seen = (SrvLines | Select-Object -Skip $n0) -match 'gstatic'
        Log "сервер ${name}: код $code, «сервер» видел запрос: $([bool]$seen)"
        if ($code -eq '204' -and $seen) { $working = $name; break }
    }
    $st = KlickJson status
    Check 'VPN (TUN) подключён' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $ad = Get-NetAdapter -Name klick -ErrorAction SilentlyContinue
    Check 'адаптер klick поднят' ($ad -and $ad.Status -eq 'Up')
    Check 'трафик системы идёт через ядро и «сервер»' ($null -ne $working) "рабочий сервер: $working"
    $dnsName = Resolve-DnsName www.gstatic.com -Type A -ErrorAction SilentlyContinue | Select-Object -First 1
    Check 'DNS отвечает подменными адресами' ("$($dnsName.IPAddress)" -like '198.18.*') "$($dnsName.IPAddress)"

    # 4a. «Как вас видят сайты» и «Сейчас в сети»
    $rep = KlickJson ip
    Check '«Как вас видят сайты»: колонка «через VPN»' ($rep.via_vpn -and $rep.via_vpn.ipv4 -and -not $rep.via_vpn.error) ("{0}, {1}, VPN заметен: {2}" -f $rep.via_vpn.country_code, $rep.via_vpn.provider, $rep.via_vpn.vpn_detected)
    Check '«Как вас видят сайты»: колонка «напрямую»' ($rep.direct.ipv4 -and -not $rep.direct.error) ("{0}, {1}" -f $rep.direct.country_code, $rep.direct.provider)
    Check '«Как вас видят сайты»: DNS под защитой kl!ck' ($rep.dns_protected -eq $true)
    Check '«Как вас видят сайты»: IPv6 не уходит мимо туннеля' ($rep.ipv6_leak -ne $true) ("ipv6_leak=" + $rep.ipv6_leak)
    $dl = Start-Process curl.exe -ArgumentList '-s', '--limit-rate', '50k', '-o', 'NUL', 'https://speed.cloudflare.com/__down?bytes=3000000' -PassThru -WindowStyle Hidden
    Start-Sleep 3
    $cf = @(KlickJson conns | Where-Object { $_.host -like '*cloudflare*' })
    Check '«Сейчас в сети» показывает соединение и маршрут' ($cf.Count -gt 0 -and $cf[0].route -eq 'vpn') ("{0} -> {1}" -f $cf[0].host, $cf[0].route)
    Stop-Process $dl -Force -ErrorAction SilentlyContinue

    # 4a'. Соседи: поддельный winws.exe (zapret) должен найтись
    New-Item -ItemType Directory -Force 'C:\fake' | Out-Null
    Copy-Item "$env:WINDIR\System32\curl.exe" 'C:\fake\winws.exe' -Force
    $fake = Start-Process 'C:\fake\winws.exe' -ArgumentList '-s', '--limit-rate', '10k', '-o', 'NUL', 'https://speed.cloudflare.com/__down?bytes=5000000' -PassThru -WindowStyle Hidden
    Start-Sleep 1
    $nb = @(KlickJson neighbors)
    Check 'соседи: zapret найден' (@($nb | Where-Object { $_.name -eq 'zapret' -and $_.conflicts_with_tun }).Count -gt 0) (($nb | ForEach-Object { "$($_.kind):$($_.name)" }) -join ', ')
    Stop-Process $fake -Force -ErrorAction SilentlyContinue

    # 4a''. Смена сети: адаптер перезапускается, служба сама проверяет связь и остаётся подключённой
    Restart-NetAdapter -Name $nic.Name -Confirm:$false -ErrorAction SilentlyContinue
    Start-Sleep 6
    $st = WaitVpn 'connected' 40
    $afterNet = Http 'curl.exe'
    Check 'после смены сети VPN снова работает' ($st.vpn -eq 'connected' -and $afterNet -eq '204') ("vpn={0}, код {1}" -f $st.vpn, $afterNet)
    # Ядро перечитывает серверы не сразу: служба не должна запомнить его заглушку COMPATIBLE.
    $saved = @((KlickJson settings).connections | Where-Object { $_.selected_server } | ForEach-Object { $_.selected_server })
    Check 'после смены сети сервер прежний' ($st.server -eq $working -and $saved -notcontains 'COMPATIBLE') ("сейчас {0}, в настройках: {1}" -f $st.server, ($saved -join ', '))

    # 4b. Переключатели России в «Всё через VPN»
    $n0 = (SrvLines).Count
    $ya = Http 'curl.exe' 'https://ya.ru'
    Start-Sleep 1
    Check '«.ru и .рф напрямую» включён: ya.ru мимо VPN' (-not ((SrvLines | Select-Object -Skip $n0) -match 'ya\.ru')) "код $ya"
    KlickCli prefs --ru-domains off --ru-ips off | Out-Null
    Start-Sleep 1
    $n0 = (SrvLines).Count
    $ya2 = Http 'curl.exe' 'https://ya.ru'
    Start-Sleep 1
    Check '«.ru и .рф напрямую» выключен: ya.ru через VPN' ([bool]((SrvLines | Select-Object -Skip $n0) -match 'ya\.ru')) "код $ya2"
    KlickCli prefs --ru-domains on --ru-ips on | Out-Null

    # 4c. «Не открывается?»: «сервер» упал — неудачные соединения видны
    Stop-Process $srvProc -Force
    Start-Sleep 1
    Http 'curl.exe' | Out-Null
    Start-Sleep 2
    $failed = @(KlickJson failures)
    Check '«Не открывается?» показывает неудачные соединения' ($failed.Count -gt 0) ("{0}: {1}" -f $failed[0].host, $failed[0].error)
    $srvProc = StartSrv
    Start-Sleep 2

    # 4d. Страж: короткий и длинный сбой тестового SOCKS5-сервера без перезапуска ядра
    KlickCli mode tun | Out-Null
    KlickCli routing all | Out-Null
    $st = WaitVpn 'connected' 25
    Check 'страж: исходное состояние — TUN и «Всё через VPN»' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $guardWatchLog = Join-Path $out 'guard-watch.jsonl'
    $guardWatchErr = Join-Path $out 'guard-watch.err'
    Remove-Item -LiteralPath $guardWatchLog, $guardWatchErr -Force -ErrorAction SilentlyContinue
    $guardWatch = Start-Process -FilePath "$inst\klick-cli.exe" -ArgumentList '--prod watch' -RedirectStandardOutput $guardWatchLog -RedirectStandardError $guardWatchErr -PassThru -WindowStyle Hidden
    Start-Sleep 1
    Check 'страж: поток событий watch запущен' (-not $guardWatch.HasExited) ("pid=" + $guardWatch.Id)
    $guardCorePid = CorePid
    Check 'страж: найден pid основного ядра' ($guardCorePid -gt 0) ("pid=$guardCorePid")

    # Отключаем сервер за 3 с до следующей плановой пробы: один-два отказа и короткие повторы до 12 с.
    $shortProbe = WaitGuardProbe 45
    Check 'страж: дождались контрольной пробы через сервер' $shortProbe ("строк сервера: " + (GuardProbeCount))
    Log 'страж: ждём 27 с, чтобы короткий сбой попал на очередную плановую пробу'
    Start-Sleep -Seconds 27
    $watchText = WatchText $guardWatchLog
    $shortDownBefore = [regex]::Matches($watchText, '"code":"vpn\.down"').Count
    $shortStart = Get-Date
    Stop-Process -Id $srvProc.Id -Force -ErrorAction SilentlyContinue
    $shortEnd = $shortStart.AddSeconds(12)
    $shortBadStates = @()
    $shortSamples = 0
    $shortPidStable = $guardCorePid -gt 0
    while ((Get-Date) -lt $shortEnd) {
        $s = KlickJson status
        $shortSamples++
        if (-not $s -or $s.vpn -ne 'connected') { $shortBadStates += $(if ($s) { $s.vpn } else { 'нет статуса' }) }
        if ((CorePid) -ne $guardCorePid) { $shortPidStable = $false }
        Start-Sleep -Milliseconds 500
    }
    $shortElapsed = ((Get-Date) - $shortStart).TotalSeconds
    $watchText = WatchText $guardWatchLog
    $shortDownAfter = [regex]::Matches($watchText, '"code":"vpn\.down"').Count
    Log ("страж watch после короткого сбоя: " + (($watchText -split "`r?`n" | Select-Object -Last 8) -join ' | '))
    Check 'страж: сбой сервера на 12 с не меняет состояние connected' ($shortBadStates.Count -eq 0) ("наблюдений: $shortSamples; другие состояния: " + (($shortBadStates | Select-Object -Unique) -join ', '))
    Check 'страж: за короткий сбой нет vpn.down' ($shortDownAfter -eq $shortDownBefore) ("vpn.down: $shortDownBefore -> $shortDownAfter; окно: {0:N1} с" -f $shortElapsed)
    Check 'страж: короткий сбой не перезапустил ядро' $shortPidStable ("pid=$guardCorePid, теперь $(CorePid)")
    $srvProc = StartSrv
    $shortServerReady = WaitSrv 20
    Check 'страж: тестовый сервер снова слушает порт 1080' $shortServerReady ("pid=$($srvProc.Id)")
    $st = WaitVpn 'connected' 30
    Check 'страж: после короткого сбоя VPN подключён' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $shortRecoveryPid = CorePid
    Check 'страж: pid ядра остался прежним после короткого сбоя' ($shortRecoveryPid -eq $guardCorePid) ("было $guardCorePid, стало $shortRecoveryPid")

    # Повторяем после свежей успешной пробы; сервер остаётся выключен 75 с.
    $longProbe = WaitGuardProbe 45
    Check 'страж: перед длинным сбоем прошла свежая контрольная проба' $longProbe ("строк сервера: " + (GuardProbeCount))
    Log 'страж: длинный сбой начнётся перед следующей 30-секундной пробой'
    Start-Sleep -Seconds 27
    $longCorePid = CorePid
    $longUnexpected = @()
    if ($longCorePid -ne $guardCorePid) { $longUnexpected += "core pid между сбоями: $guardCorePid -> $longCorePid" }
    $watchText = WatchText $guardWatchLog
    $longDownBefore = [regex]::Matches($watchText, '"code":"vpn\.down"').Count
    $longRestoredBefore = [regex]::Matches($watchText, '"code":"vpn\.restored"').Count
    $longStart = Get-Date
    Stop-Process -Id $srvProc.Id -Force -ErrorAction SilentlyContinue
    $longTimer = [Diagnostics.Stopwatch]::StartNew()
    $reconnectingAt = $null
    $serverDownAt = $null
    $downNoticeAt = $null
    $lastPidSample = 0.0
    while ($longTimer.Elapsed.TotalSeconds -lt 75) {
        $elapsed = $longTimer.Elapsed.TotalSeconds
        $s = KlickJson status
        if ($s.vpn -eq 'reconnecting' -and $null -eq $reconnectingAt) { $reconnectingAt = $elapsed }
        if ($s.vpn -eq 'server_down' -and $null -eq $serverDownAt) { $serverDownAt = $elapsed }
        if ($s.vpn -notin @('connected', 'reconnecting', 'server_down')) { $longUnexpected += "$($s.vpn)@{0:N1}s" -f $elapsed }
        $watchText = WatchText $guardWatchLog
        $downCountNow = [regex]::Matches($watchText, '"code":"vpn\.down"').Count
        if ($null -eq $downNoticeAt -and $downCountNow -gt $longDownBefore) { $downNoticeAt = $elapsed }
        if ($elapsed -ge $lastPidSample) {
            if ((CorePid) -ne $longCorePid) { $longUnexpected += "core pid $(CorePid)@$('{0:N1}' -f $elapsed)s" }
            $lastPidSample = $elapsed + 1.0
        }
        Start-Sleep -Milliseconds 500
    }
    $longTimer.Stop()
    $longHeld = $longTimer.Elapsed.TotalSeconds
    Check 'страж: reconnecting появляется не раньше 15 с' ($null -ne $reconnectingAt -and $reconnectingAt -ge 15) ("момент: {0:N1} с" -f $reconnectingAt)
    Check 'страж: server_down появляется не позже 60 с' ($null -ne $serverDownAt -and $serverDownAt -le 60) ("момент: {0:N1} с" -f $serverDownAt)
    Check 'страж: watch получил vpn.down не позже 60 с' ($null -ne $downNoticeAt -and $downNoticeAt -le 60) ("момент: {0:N1} с" -f $downNoticeAt)
    Check 'страж: длинный сбой длился около 75 с' ($longHeld -ge 74 -and $longHeld -le 85) ("окно: {0:N1} с" -f $longHeld)

    $srvProc = StartSrv
    $longServerReady = WaitSrv 20
    Check 'страж: после длинного сбоя тестовый сервер снова слушает порт 1080' $longServerReady ("pid=$($srvProc.Id)")
    $restoreTimer = [Diagnostics.Stopwatch]::StartNew()
    $restoredState = $null
    $restoredNoticeAt = $null
    while ($restoreTimer.Elapsed.TotalSeconds -lt 90) {
        $restoredState = KlickJson status
        $watchText = WatchText $guardWatchLog
        $restoredCount = [regex]::Matches($watchText, '"code":"vpn\.restored"').Count
        if ($null -eq $restoredNoticeAt -and $restoredCount -gt $longRestoredBefore) { $restoredNoticeAt = $restoreTimer.Elapsed.TotalSeconds }
        if ((CorePid) -ne $longCorePid) { $longUnexpected += "core pid $(CorePid) после восстановления" }
        if ($restoredState.vpn -eq 'connected' -and $null -ne $restoredNoticeAt) { break }
        Start-Sleep -Milliseconds 500
    }
    $restoreTimer.Stop()
    $watchText = WatchText $guardWatchLog
    $longDownAfter = [regex]::Matches($watchText, '"code":"vpn\.down"').Count
    $longRestoredAfter = [regex]::Matches($watchText, '"code":"vpn\.restored"').Count
    Log ("страж watch после длинного сбоя: " + (($watchText -split "`r?`n" | Select-Object -Last 12) -join ' | '))
    Check 'страж: после восстановления VPN вернулся в connected за 90 с' ($restoredState.vpn -eq 'connected' -and $restoreTimer.Elapsed.TotalSeconds -le 90) ("vpn=$($restoredState.vpn), время: {0:N1} с" -f $restoreTimer.Elapsed.TotalSeconds)
    Check 'страж: watch получил vpn.down и vpn.restored' ($longDownAfter -gt $longDownBefore -and $longRestoredAfter -gt $longRestoredBefore) ("vpn.down: $longDownBefore -> $longDownAfter; vpn.restored: $longRestoredBefore -> $longRestoredAfter")
    Check 'страж: pid основного ядра не менялся во время сбоя и восстановления' ($longCorePid -gt 0 -and $longUnexpected.Count -eq 0 -and (CorePid) -eq $longCorePid) ("pid=$longCorePid; отклонения: " + (($longUnexpected | Select-Object -Unique) -join ', '))
    if ($guardWatch -and -not $guardWatch.HasExited) { Stop-Process -Id $guardWatch.Id -Force -ErrorAction SilentlyContinue }
    $guardWatch = $null

    # 5. «Только выбранное»: обычный сайт напрямую, заблокированный сервис через VPN
    KlickCli routing selected | Out-Null
    Start-Sleep 1
    # Обычный сайт — не www.gstatic.com: через него служба сама проверяет связь через VPN (PROBE_URL),
    # и её проверка, совпавшая по времени (например, после смены сети), попадала в журнал «сервера».
    $n0 = (SrvLines).Count
    # Обычный сайт — не gstatic: через «сервер» его раз в 30 с запрашивает проверка связи самого ядра.
    $sel = Http 'curl.exe' 'https://www.wikipedia.org/'
    $yt = Http 'curl.exe' 'https://www.youtube.com/generate_204'
    Start-Sleep 1
    $new = SrvLines | Select-Object -Skip $n0
    Check 'в «Только выбранное» обычный сайт идёт напрямую' ($sel -match '^(200|301|302)$' -and -not ($new -match 'wikipedia')) "код $sel"
    # «Сервер» выходит в интернет отсюда же, а заблокированное в РФ отсюда не открывается:
    # проверяем, что запрос ушёл через «сервер», а не код ответа сайта.
    Check 'сервис из набора заблокированного идёт через VPN' ([bool]($new -match 'youtube')) "код $yt"

    # 5b. TUN, программа через VPN и тот же домен напрямую по правилу программы
    KlickCli routing selected | Out-Null
    $discordFolder = 'C:\apptest\Discord\app-1.0.9001'
    New-Item -ItemType Directory -Force $discordFolder | Out-Null
    $discordCurl = Join-Path $discordFolder 'Discord.exe'
    Copy-Item "$env:WINDIR\System32\curl.exe" $discordCurl -Force
    $programAdd = KlickCli list add selected program $discordFolder vpn
    $programAddExit = $LASTEXITCODE
    Check 'правило Discord добавлено в «Только выбранное»' ($programAddExit -eq 0) ($programAdd -replace "`r?`n", ' ')
    $domainAdd = KlickCli list add selected domain 'speed.cloudflare.com' direct
    $domainAddExit = $LASTEXITCODE
    Check 'speed.cloudflare.com добавлен как domain direct' ($domainAddExit -eq 0) ($domainAdd -replace "`r?`n", ' ')

    $programLogStart = (SrvLines).Count
    $discordDownload = StartTestDownload $discordCurl (Join-Path $out 'discord-route.bin') (Join-Path $out 'discord-route.err')
    $discordConn = WaitConn 'Discord.exe' 'vpn' 20
    Start-Sleep 1
    $programServerHits = @((SrvLines | Select-Object -Skip $programLogStart) | Where-Object { $_ -match 'speed\.cloudflare\.com' }).Count
    $discordErrTail = ((Get-Content (Join-Path $out 'discord-route.err') -Tail 8 -ErrorAction SilentlyContinue) -join ' | ')
    $discordDetail = if ($discordConn) { "process=$($discordConn.process); path=$($discordConn.process_path); route=$($discordConn.route); rule=$($discordConn.rule)" } else { "нет соединения; conns=" + ((KlickJson conns | ConvertTo-Json -Depth 5 -Compress)) + "; curl: $discordErrTail" }
    Check 'TUN: Discord.exe идёт через VPN, несмотря на domain direct' ($null -ne $discordConn) $discordDetail
    $programServerTail = ((SrvLines | Select-Object -Last 8) -join ' | ')
    Check 'TUN: тестовый VPN-сервер увидел запрос Discord.exe' ($programServerHits -gt 0) ("запросов speed.cloudflare.com: $programServerHits; журнал: $programServerTail")

    $systemLogStart = (SrvLines).Count
    $systemDownload = StartTestDownload "$env:WINDIR\System32\curl.exe" (Join-Path $out 'system-curl-route.bin') (Join-Path $out 'system-curl-route.err')
    $systemConn = WaitConn 'curl.exe' 'direct' 20
    Start-Sleep 1
    $systemServerHits = @((SrvLines | Select-Object -Skip $systemLogStart) | Where-Object { $_ -match 'speed\.cloudflare\.com' }).Count
    $systemErrTail = ((Get-Content (Join-Path $out 'system-curl-route.err') -Tail 8 -ErrorAction SilentlyContinue) -join ' | ')
    $systemDetail = if ($systemConn) { "process=$($systemConn.process); path=$($systemConn.process_path); route=$($systemConn.route); rule=$($systemConn.rule)" } else { "нет соединения; conns=" + ((KlickJson conns | ConvertTo-Json -Depth 5 -Compress)) + "; curl: $systemErrTail" }
    Check 'TUN: системный curl.exe идёт напрямую' ($null -ne $systemConn) $systemDetail
    $systemServerTail = ((SrvLines | Select-Object -Last 8) -join ' | ')
    Check 'TUN: системный curl.exe не попал на тестовый VPN-сервер' ($systemServerHits -eq 0) ("новых запросов speed.cloudflare.com: $systemServerHits; журнал: $systemServerTail")
    foreach ($p in @($discordDownload, $systemDownload)) {
        if ($p -and -not $p.HasExited) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
    }

    # Меняем список, пока через программу идёт ограниченная по скорости загрузка.
    $longDownloadPath = Join-Path $out 'list-change-download.bin'
    $longDownload = StartTestDownload $discordCurl $longDownloadPath (Join-Path $out 'list-change-download.err') '20k'
    $bytesReady = $false
    $bytesDeadline = (Get-Date).AddSeconds(25)
    while ((Get-Date) -lt $bytesDeadline) {
        $f = Get-Item -LiteralPath $longDownloadPath -ErrorAction SilentlyContinue
        if ($f -and $f.Length -gt 0) { $bytesReady = $true; break }
        $longDownload.Refresh()
        if ($longDownload.HasExited) { break }
        Start-Sleep 1
    }
    $beforeBytes = [long]0
    $f = Get-Item -LiteralPath $longDownloadPath -ErrorAction SilentlyContinue
    if ($f) { $beforeBytes = [long]$f.Length }
    $reloadRule = KlickCli list add selected domain 'reload-check.klick.test' direct
    $reloadRuleExit = $LASTEXITCODE
    Check 'изменение списка для проверки reload принято' ($reloadRuleExit -eq 0) ($reloadRule -replace "`r?`n", ' ')
    Start-Sleep 8
    $afterBytes = [long]0
    $f = Get-Item -LiteralPath $longDownloadPath -ErrorAction SilentlyContinue
    if ($f) { $afterBytes = [long]$f.Length }
    $longDownload.Refresh()
    if ($longDownload.HasExited) {
        $downloadOutcome = "curl завершился с кодом $($longDownload.ExitCode)"
    } else {
        $downloadOutcome = 'curl всё ещё работает'
    }
    $growth = $afterBytes - $beforeBytes
    $longErrTail = ((Get-Content (Join-Path $out 'list-change-download.err') -Tail 8 -ErrorAction SilentlyContinue) -join ' | ')
    Log ("INFO соединение Discord.exe при reload: поток до reload был {0}; байты {1} -> {2} (+{3}); {4}; curl: {5}" -f $bytesReady, $beforeBytes, $afterBytes, $growth, $downloadOutcome, $longErrTail)
    if (-not $longDownload.HasExited) { Stop-Process -Id $longDownload.Id -Force -ErrorAction SilentlyContinue }
    KlickCli routing all | Out-Null

    # 6. Kill Switch
    New-Item -ItemType Directory -Force 'C:\kstest' | Out-Null
    Copy-Item "$env:WINDIR\System32\curl.exe" 'C:\kstest\curl.exe' -Force
    Log ("ks add: " + (KlickCli ks add 'C:\kstest'))
    $ksOn = Http 'C:\kstest\curl.exe'
    Check 'Kill Switch: программа работает через туннель' ($ksOn -eq '204') "код $ksOn"
    KlickCli disconnect | Out-Null
    Start-Sleep 2
    Check 'адаптер исчез после отключения' (-not (Get-NetAdapter -Name klick -ErrorAction SilentlyContinue))
    $ksOff = Http 'C:\kstest\curl.exe'
    Check 'Kill Switch: без VPN программа без сети' ($ksOff -eq '000') "код $ksOff"
    $ksStatus = @(KlickJson ks status)
    Check 'Kill Switch видит exe в папке программы' ($ksStatus.Count -eq 1 -and $ksStatus[0].exes -ge 1) ("exe: " + $ksStatus[0].exes)
    KlickCli ks program 'C:\kstest' off | Out-Null
    Start-Sleep 1
    $ksPaused = Http 'C:\kstest\curl.exe'
    Check 'Kill Switch: выключенная в списке программа ходит напрямую' ($ksPaused -eq '204') "код $ksPaused"
    KlickCli ks program 'C:\kstest' on | Out-Null
    Start-Sleep 1
    $ksBack = Http 'C:\kstest\curl.exe'
    Check 'Kill Switch: включённая снова программа без сети' ($ksBack -eq '000') "код $ksBack"
    # Программа обновилась в новую папку (так делают Roblox, Discord): новый exe закрыт сразу, а не через 10 минут.
    New-Item -ItemType Directory -Force 'C:\kstest\version-2' | Out-Null
    Copy-Item "$env:WINDIR\System32\curl.exe" 'C:\kstest\version-2\curl.exe' -Force
    Start-Sleep 2
    $ksNew = Http 'C:\kstest\version-2\curl.exe'
    Check 'Kill Switch: новая версия программы без сети за 2 с' ($ksNew -eq '000') "код $ksNew"
    # Установщик собрал папку версии в другом месте и переложил целиком.
    New-Item -ItemType Directory -Force 'C:\kstage' | Out-Null
    Copy-Item "$env:WINDIR\System32\curl.exe" 'C:\kstage\curl.exe' -Force
    Move-Item 'C:\kstage' 'C:\kstest\version-3'
    Start-Sleep 2
    $ksMoved = Http 'C:\kstest\version-3\curl.exe'
    Check 'Kill Switch: переложенная целиком папка версии без сети за 2 с' ($ksMoved -eq '000') "код $ksMoved"
    Remove-Item 'C:\kstest\version-2', 'C:\kstest\version-3' -Recurse -Force
    $sysOff = Http 'curl.exe'
    Check 'Kill Switch не мешает остальным программам' ($sysOff -eq '204') "код $sysOff"
    $lan = Http 'C:\kstest\curl.exe' 'https://www.gstatic.com/generate_204' @('--socks5-hostname', "${ip}:1080")
    Check 'Kill Switch пропускает локальную сеть' ($lan -eq '204') "код $lan"

    # 7. Системный прокси: программа из Kill Switch ходит только через порт 7890
    KlickCli mode proxy | Out-Null
    KlickCli connect | Out-Null
    Start-Sleep 2
    $viaPort = Http 'C:\kstest\curl.exe' 'https://www.gstatic.com/generate_204' @('-x', 'http://127.0.0.1:7890')
    Check 'системный прокси: программа из Kill Switch работает через порт 7890' ($viaPort -eq '204') "код $viaPort"
    $noPort = Http 'C:\kstest\curl.exe'
    Check 'системный прокси: мимо порта программа из Kill Switch без сети' ($noPort -eq '000') "код $noPort"
    # Служба замечает такую программу по событиям брандмауэра: ей нужен режим VPN (TUN).
    $np = @()
    for ($i = 0; $i -lt 15 -and -not $np.Count; $i++) { Start-Sleep 1; $np = @(KlickJson ks status | Where-Object { $_.no_proxy }) }
    Check 'системный прокси: служба заметила программу, которая не использует прокси' ($np.Count -eq 1 -and $np[0].folder -eq 'C:\kstest') (($np | ForEach-Object { $_.folder }) -join ', ')
    $reg = ProxyReg
    Check 'без окна системный прокси пользователю поставила служба' ($reg.ProxyEnable -eq 1 -and $reg.ProxyServer -eq '127.0.0.1:7890') ("ProxyEnable={0}, ProxyServer={1}" -f $reg.ProxyEnable, $reg.ProxyServer)
    KlickCli disconnect | Out-Null
    Start-Sleep 1
    $reg = ProxyReg
    Check 'без окна служба сняла системный прокси' ($reg.ProxyEnable -eq 0) ("ProxyEnable=" + $reg.ProxyEnable)

    # 7б. Системный прокси ставит и снимает окно kl!ck
    $backup = Join-Path $env:LOCALAPPDATA 'klick\proxy-backup.json'
    $seen = WindowLinks
    $winProc = Start-Process "$inst\klick.exe" -ArgumentList '--hidden' -RedirectStandardError "$out\ui.err" -PassThru
    $up = WaitWindow $seen
    Check 'окно kl!ck запустилось в трее и на связи со службой' (-not $winProc.HasExited -and $up)
    KlickCli connect | Out-Null
    Start-Sleep 3
    $reg = ProxyReg
    Check 'окно поставило системный прокси и запомнило прежний' ($reg.ProxyEnable -eq 1 -and $reg.ProxyServer -eq '127.0.0.1:7890' -and (Test-Path $backup)) ("ProxyEnable={0}, ProxyServer={1}, копия: {2}" -f $reg.ProxyEnable, $reg.ProxyServer, (Test-Path $backup))
    KlickCli disconnect | Out-Null
    Start-Sleep 2
    $reg = ProxyReg
    Check 'окно сняло прокси и вернуло прежние настройки' ($reg.ProxyEnable -eq 0 -and -not (Test-Path $backup)) ("ProxyEnable={0}, копия: {1}" -f $reg.ProxyEnable, (Test-Path $backup))
    # Чужой прокси на том же порту (так по умолчанию у Clash for Windows) kl!ck не трогает.
    $key = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
    Set-ItemProperty $key ProxyServer '127.0.0.1:7890'
    Set-ItemProperty $key ProxyEnable 1
    Stop-Process -Id $winProc.Id -Force -ErrorAction SilentlyContinue
    # Вторая копия окна, пока первая ещё не закрылась, передаёт ей аргументы и выходит — дождаться.
    Wait-Process -Id $winProc.Id -Timeout 15 -ErrorAction SilentlyContinue
    $seen = WindowLinks
    $winProc = Start-Process "$inst\klick.exe" -ArgumentList '--hidden' -RedirectStandardError "$out\ui2.err" -PassThru
    $null = WaitWindow $seen
    Start-Sleep 2
    $reg = ProxyReg
    Check 'чужой прокси на 127.0.0.1:7890 kl!ck не снимает' ($reg.ProxyEnable -eq 1) ("ProxyEnable=" + $reg.ProxyEnable)
    Set-ItemProperty $key ProxyEnable 0
    Stop-Process -Id $winProc.Id -Force -ErrorAction SilentlyContinue

    # 8. Падение ядра
    KlickCli mode tun | Out-Null
    KlickCli routing all | Out-Null
    KlickCli connect | Out-Null
    Start-Sleep 3
    $core = CoreProc
    Log "ядро pid $($core.ProcessId); убиваю"
    if ($core) { Stop-Process -Id $core.ProcessId -Force }
    Start-Sleep 5
    $core2 = CoreProc
    $st = KlickJson status
    Check 'служба перезапустила упавшее ядро' ($core2 -and $core2.ProcessId -ne $core.ProcessId -and $st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $after = Http 'C:\kstest\curl.exe'
    Check 'после перезапуска ядра Kill Switch пропускает через новый адаптер' ($after -eq '204') "код $after"

    # 9. Падение службы
    $svcProc = Get-Process klick-service -ErrorAction SilentlyContinue
    Log "служба pid $($svcProc.Id); убиваю"
    if ($svcProc) { Stop-Process -Id $svcProc.Id -Force }
    Start-Sleep 1
    Check 'ядро завершилось вместе со службой' (-not (CoreProc))
    $ksDown = Http 'C:\kstest\curl.exe'
    Check 'Kill Switch держится, пока службы нет' ($ksDown -eq '000') "код $ksDown"
    Start-Sleep 5
    Check 'Windows перезапустила службу' ((Get-Service klick).Status -eq 'Running')
    $st = WaitVpn 'connected' 20
    $restored = Http 'C:\kstest\curl.exe'
    Check 'после сбоя службы VPN вернулся сам' ($st.vpn -eq 'connected' -and $restored -eq '204') ("vpn={0}, код {1}" -f $st.vpn, $restored)
    KlickCli disconnect | Out-Null

    # 10. «Восстанавливать подключение»: VPN был включён, когда Windows «перезагрузили».
    # Перезагрузку изображаем записью службы о прошлом запуске с чужим временем загрузки.
    KlickCli prefs --restore on | Out-Null
    Stop-Service klick
    Set-Content 'C:\ProgramData\klick\runtime.json' '{"connected":true,"boot":1000}' -Encoding ASCII
    Start-Service klick
    Start-Sleep 3
    $st = KlickJson status
    Check 'после перезагрузки служба сама VPN не включает' ($st.vpn -eq 'off') ("vpn=" + $st.vpn)
    KlickCli resume | Out-Null
    $st = WaitVpn 'connected' 20
    Check 'после входа окно вернуло VPN («Восстанавливать подключение»)' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    KlickCli disconnect | Out-Null
    KlickCli resume | Out-Null
    Start-Sleep 2
    $st = KlickJson status
    Check 'второй вызов resume ничего не включает' ($st.vpn -eq 'off') ("vpn=" + $st.vpn)

    # 11. Журнал, отчёт, «О приложении»
    $log = @(KlickJson log)
    Check 'журнал службы отдаётся окну' ($log.Count -gt 5) ("записей: " + $log.Count)
    $rep = KlickCli report
    Check 'отчёт без адресов «сервера»' ($rep -match 'kl!ck' -and -not ($rep -match [regex]::Escape($ip))) ("строк: " + ($rep -split "`n").Count)
    $about = KlickJson about
    Check '«О приложении»: версия ядра и система' ($about.core_version -like 'v*' -and $about.os -like 'Windows*') ("{0} · {1}" -f $about.core_version, $about.os)

    # 12. Удаление
    if ($KeepInstalled) { return }
    Log ("cleanup-wfp: " + ((& "$inst\klick-service.exe" cleanup-wfp 2>&1 | Out-String).Trim()))
    Log ("uninstall: " + ((& "$inst\klick-service.exe" uninstall 2>&1 | Out-String).Trim()))
    Start-Sleep 1
    Check 'служба удалена' (-not (Get-Service klick -ErrorAction SilentlyContinue))
    $ksAfter = Http 'C:\kstest\curl.exe'
    Check 'после удаления программа снова в сети' ($ksAfter -eq '204') "код $ksAfter"
}
catch {
    $script:fails++
    Log "ОШИБКА СЦЕНАРИЯ: $($_.Exception.Message)"
}
finally {
    if ($guardWatch -and -not $guardWatch.HasExited) { Stop-Process -Id $guardWatch.Id -Force -ErrorAction SilentlyContinue }
    Log ("ИТОГ: провалов {0}" -f $script:fails)
    Copy-Item 'C:\ProgramData\klick\logs\service.log' "$out\service.log" -ErrorAction SilentlyContinue
    Copy-Item 'C:\ProgramData\klick\core\config.yaml' "$out\core-config.yaml" -ErrorAction SilentlyContinue
    # Из finally выйти через return нельзя — поэтому if/else.
    if ($KeepInstalled) {
        if ($srvProc) { Stop-Process -Id $srvProc.Id -Force -ErrorAction SilentlyContinue }
    } else {
        Get-Process mihomo -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
        Set-Content "$out\done.txt" 'done'
        if (-not $NoShutdown) {
            Start-Sleep 2
            shutdown.exe /s /t 0
        }
    }
}
