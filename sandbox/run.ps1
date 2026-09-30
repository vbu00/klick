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

function Log([string]$m) { Add-Content -Path $report -Value ("[{0:HH:mm:ss}] {1}" -f (Get-Date), $m) -Encoding UTF8 }
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if (-not $ok) { $script:fails++ }
    $mark = if ($ok) { 'OK  ' } else { 'FAIL' }
    $tail = if ($detail) { " - $detail" } else { '' }
    Log "$mark $name$tail"
}
function KlickCli { (& "$inst\klick-cli.exe" --prod @args 2>&1 | Out-String).Trim() }
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
    $reg = ProxyReg
    Check 'без окна системный прокси пользователю поставила служба' ($reg.ProxyEnable -eq 1 -and $reg.ProxyServer -eq '127.0.0.1:7890') ("ProxyEnable={0}, ProxyServer={1}" -f $reg.ProxyEnable, $reg.ProxyServer)
    KlickCli disconnect | Out-Null
    Start-Sleep 1
    $reg = ProxyReg
    Check 'без окна служба сняла системный прокси' ($reg.ProxyEnable -eq 0) ("ProxyEnable=" + $reg.ProxyEnable)

    # 7б. Системный прокси ставит и снимает окно kl!ck
    $backup = Join-Path $env:LOCALAPPDATA 'klick\proxy-backup.json'
    $winProc = Start-Process "$inst\klick.exe" -ArgumentList '--hidden' -RedirectStandardError "$out\ui.err" -PassThru
    Start-Sleep 6
    Check 'окно kl!ck запустилось в трее' (-not $winProc.HasExited)
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
    $winProc = Start-Process "$inst\klick.exe" -ArgumentList '--hidden' -RedirectStandardError "$out\ui2.err" -PassThru
    Start-Sleep 6
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
