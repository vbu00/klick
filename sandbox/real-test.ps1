# Проверка kl!ck с настоящими серверами из подписки — на раннере CI (sandbox\ci.ps1) или тестовом компьютере.
# Ссылка подписки — в переменной KLICK_TEST_SUB, в отчёт она не пишется. kl!ck ставится установщиком,
# в конце удаляется вместе с данными. Сценарии: та же подписка второй раз и ссылкой klick://add («Уже
# добавлено»), прокси и VPN (TUN) «всё через VPN», системный прокси для
# обычных программ, UDP, DNS, смена сервера, по 6 включений и выключений в каждом режиме, смена режима на ходу,
# 3 минуты под нагрузкой (обрывы, перезапуски ядра, память), сеть без зашифрованного DNS, Kill Switch со сбоями
# ядра и службы, скорость «Отключить», удаление при включённом VPN.
# Меняет настройки сети — только на тестовой машине.
param([switch]$NoShutdown)

$ErrorActionPreference = 'Continue'
$root = 'C:\klick'
$out = Join-Path $root 'results'
$inst = 'C:\Program Files\klick'
New-Item -ItemType Directory -Force $out | Out-Null
$report = Join-Path $out 'real-report.txt'
Set-Content -Path $report -Value '' -Encoding UTF8
$script:fails = 0
$sub = $env:KLICK_TEST_SUB

function Log([string]$m) { Add-Content -Path $report -Value ("[{0:HH:mm:ss}] {1}" -f (Get-Date), $m) -Encoding UTF8 }
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if (-not $ok) { $script:fails++ }
    $mark = if ($ok) { 'OK  ' } else { 'FAIL' }
    $tail = if ($detail) { " - $detail" } else { '' }
    Log "$mark $name$tail"
}
function KlickCli { (& "$inst\klick-cli.exe" --prod @args 2>&1 | Out-String).Trim() }
function KlickJson { $t = & "$inst\klick-cli.exe" --prod @args 2>$null | Out-String; try { $t | ConvertFrom-Json | ForEach-Object { $_ } } catch { $null } }
function WaitVpn([string]$want, [int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    do { $s = KlickJson status; if ($s.vpn -eq $want) { return $s }; Start-Sleep -Milliseconds 250 } while ((Get-Date) -lt $end)
    return $s
}
function Http([string]$exe = 'curl.exe', [string]$url = 'https://www.gstatic.com/generate_204', [string[]]$extra = @()) {
    $code = & $exe -s -o NUL -w '%{http_code}' --max-time 12 @extra $url 2>$null
    if ($code) { "$code".Trim() } else { '000' }
}
# Адрес выхода по TCP: два независимых сервиса, первый ответивший.
function IpVia([string]$exe = 'curl.exe', [string[]]$extra = @(), [int]$timeout = 10) {
    foreach ($u in 'https://api.ipify.org', 'https://ipv4.icanhazip.com') {
        $r = (& $exe -4 -s --max-time $timeout @extra $u 2>$null | Out-String).Trim()
        if ($r -match '^\d+\.\d+\.\d+\.\d+$') { return $r }
    }
    return ''
}
# Адрес раннера в NAT облака меняется в пределах /24 — прямым считаем любой адрес из той же /24.
function Net24([string]$ip) { if ($ip) { $ip.Substring(0, $ip.LastIndexOf('.')) } else { '' } }
function IsDirect([string]$ip) { [bool]$ip -and (Net24 $ip) -eq (Net24 $script:direct) }
function Speed([string[]]$extra = @()) {
    $bps = & curl.exe -s -o NUL --max-time 20 -w '%{speed_download}' @extra 'https://speed.cloudflare.com/__down?bytes=25000000' 2>$null
    $v = 0.0
    if ([double]::TryParse("$bps".Trim().Replace(',', '.'), [Globalization.NumberStyles]::Float, [Globalization.CultureInfo]::InvariantCulture, [ref]$v)) {
        Log ("    скорость: {0:N1} Мбит/с" -f ($v * 8 / 1e6))
    }
}
# Адрес выхода по UDP: запрос STUN (так его видят звонки и игры). Нужен Python; без него — пусто.
$stun = @'
import os, socket, struct, sys
req = struct.pack('!HHI', 1, 0, 0x2112A442) + os.urandom(12)
for host, port in (('stun.l.google.com', 19302), ('stun.cloudflare.com', 3478)):
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.settimeout(3)
        s.sendto(req, (host, port))
        data = s.recv(2048)
        i = 20
        while i + 4 <= len(data):
            t, l = struct.unpack('!HH', data[i:i + 4])
            v = data[i + 4:i + 4 + l]
            if t in (0x0020, 0x0001) and len(v) >= 8:
                ip = bytes(v[4:8])
                if t == 0x0020:
                    ip = bytes(a ^ b for a, b in zip(ip, struct.pack('!I', 0x2112A442)))
                print('.'.join(map(str, ip)))
                sys.exit(0)
            i += 4 + l + (-l % 4)
    except Exception:
        pass
sys.exit(1)
'@
Set-Content "$out\stun.py" $stun -Encoding ASCII
$python = (Get-Command python.exe -ErrorAction SilentlyContinue).Source
function UdpIp { if ($python) { (& $python "$out\stun.py" 2>$null | Out-String).Trim() } else { '' } }
# Отключить и замерить: «Отключить» должно срабатывать сразу, а не через секунды.
function DisconnectTimed([string]$what) {
    $t = Measure-Command { KlickCli disconnect | Out-Null }
    $line = Get-Content 'C:\ProgramData\klick\logs\service.log' -Encoding UTF8 -ErrorAction SilentlyContinue | Select-String 'отключено за' | Select-Object -Last 1
    Log ("    «Отключить» ({0}): {1:N3} с {2}" -f $what, $t.TotalSeconds, $(if ($line) { '; ' + ($line.Line -replace '.*отключено', 'отключено') } else { '' }))
    Check "«Отключить» ($what) быстрее 2 с" ($t.TotalSeconds -le 2) ("{0:N3} с" -f $t.TotalSeconds)
}
# Программа из Kill Switch ни разу не вышла в интернет напрямую за столько-то секунд.
function KsNeverDirect([int]$seconds) {
    $end = (Get-Date).AddSeconds($seconds)
    while ((Get-Date) -lt $end) {
        $ip = (& 'C:\kstest\curl.exe' -4 -s --max-time 2 'https://api.ipify.org' 2>$null | Out-String).Trim()
        if ($ip -match '^\d+\.\d+\.\d+\.\d+$' -and (IsDirect $ip)) { Log "    программа из Kill Switch вышла напрямую: $ip"; return $false }
        Start-Sleep -Milliseconds 200
    }
    return $true
}
function KsViaVpn { $ip = IpVia 'C:\kstest\curl.exe'; [bool]$ip -and -not (IsDirect $ip) }
function ProxyReg { Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' }
# Основное ядро (config.yaml); проверочное — с tester.yaml, страж — с guard.
function CoreProc { Get-CimInstance Win32_Process -Filter "Name='mihomo.exe'" | Where-Object { $_.CommandLine -like '*ProgramData*' -and $_.CommandLine -like '*config.yaml*' -and $_.CommandLine -notlike '*guard*' } }
# Адрес выхода, как его видит обычная программа с системным прокси (.NET читает настройки Windows).
# Отдельный процесс: настройки прокси процесс читает один раз.
function IpSystemProxy {
    $r = & "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -Command "try { (Invoke-WebRequest -UseBasicParsing -TimeoutSec 10 'https://api.ipify.org').Content } catch { '' }" 2>$null
    $r = "$r".Trim()
    if ($r -match '^\d+\.\d+\.\d+\.\d+$') { $r } else { '' }
}
. "$PSScriptRoot\ui.ps1"
function Mb([int]$id) { $p = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($p) { [math]::Round($p.WorkingSet64 / 1MB) } else { 0 } }

try {
    if (-not $sub) { throw 'нет KLICK_TEST_SUB' }
    Log ("Windows {0}, {1}" -f [Environment]::OSVersion.Version, (Get-CimInstance Win32_OperatingSystem).Caption)
    $script:direct = IpVia
    Check 'интернет напрямую есть' ([bool]$script:direct) $script:direct
    $directUdp = UdpIp
    Log "    STUN напрямую: $(if ($directUdp) { 'отвечает' } else { 'нет ответа или нет Python' })"

    # 1. Установка установщиком, как у человека
    $p = Start-Process "$root\klick-setup.exe" -ArgumentList '--silent', '--no-autostart' -Wait -PassThru -WindowStyle Hidden
    Check 'установка: код 0' ($p.ExitCode -eq 0) "код $($p.ExitCode)"
    Copy-Item "$root\bin\klick-cli.exe" $inst -Force
    $end = (Get-Date).AddSeconds(20)
    while (-not (KlickJson status) -and (Get-Date) -lt $end) { Start-Sleep 1 }
    Check 'служба отвечает' ($null -ne (KlickJson status))

    # 2. Подписка, серверы, задержка
    $added = KlickCli add $sub
    Check 'подписка добавлена' ($LASTEXITCODE -eq 0 -and $added -notmatch 'error|ошибк') (($added -replace [regex]::Escape($sub), '<ссылка>') -split "`n" | Select-Object -First 1)
    # Та же подписка второй раз — не дубль: из командной строки и ссылкой klick://add со страницы подписки.
    $again = KlickCli add $sub
    Check 'та же подписка второй раз: «уже добавлено»' ($again -match 'conn\.exists') (($again -replace [regex]::Escape($sub), '<ссылка>') -split "`n" | Select-Object -First 1)
    Start-Process "$inst\klick.exe"
    $win = WaitKlick 30
    if ($win) {
        $null = WaitUi $win.Id @('Главная') 40
        Start-Process ('klick://add?url=' + [uri]::EscapeDataString($sub))
        $ui = WaitUi $win.Id @('Уже добавлено') 15
        Check 'ссылка klick://add на неё же: «Уже добавлено», экран «Добавить» не открылся' ($ui.Contains('Уже добавлено') -and -not $ui.Contains('Добавить подписку'))
        Check 'ссылки подписки в окне нет' (-not $ui.Contains($sub))
        Check 'окно kl!ck одно' ((KlickProcs).Count -eq 1) ("klick.exe: " + (KlickProcs).Count)
    } else { Check 'окно kl!ck запустилось' $false }
    KlickProcs | Stop-Process -Force -ErrorAction SilentlyContinue
    KlickProcs | Wait-Process -Timeout 15 -ErrorAction SilentlyContinue
    $n = @((KlickJson settings).connections).Count
    Check 'подключение одно, дубля нет' ($n -eq 1) "подключений: $n"
    $servers = @(KlickJson servers)
    Log ("    серверов: {0} · {1}" -f $servers.Count, ((@($servers | ForEach-Object { $_.kind }) | Sort-Object -Unique) -join ', '))
    $lat = @(KlickJson latency)
    foreach ($s in $lat) { Log ("    {0,-24} {1}" -f $s.name, $(if ($s.delay) { "$($s.delay) мс" } else { 'нет ответа' })) }
    $fast = @($lat | Where-Object { $_.delay -gt 0 } | Sort-Object delay | ForEach-Object { $_.name })
    Check 'задержка измерена хотя бы у одного' ($fast.Count -gt 0)
    if ($fast.Count -eq 0) { throw 'ни один сервер не ответил — дальше проверять нечего' }
    $sel = KlickCli server $fast[0]
    Check "выбран самый быстрый: $($fast[0])" ($LASTEXITCODE -eq 0) $sel

    # 3. «Системный прокси», всё через VPN
    KlickCli routing all | Out-Null
    KlickCli mode proxy | Out-Null
    KlickCli connect | Out-Null
    $st = WaitVpn 'connected' 30
    Check 'прокси: подключено' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $vpn = IpVia 'curl.exe' @('-x', 'http://127.0.0.1:7890')
    Check 'через порт 7890 адрес выхода — сервера, не компьютера' ([bool]$vpn -and -not (IsDirect $vpn))
    $reg = ProxyReg
    Check 'системный прокси поставлен' ($reg.ProxyEnable -eq 1 -and $reg.ProxyServer -eq '127.0.0.1:7890') ("ProxyEnable={0}, ProxyServer={1}" -f $reg.ProxyEnable, $reg.ProxyServer)
    $sys = IpSystemProxy
    Check 'обычные программы с системным прокси (.NET) идут через VPN' ([bool]$sys -and -not (IsDirect $sys)) $(if ($sys) { '' } else { 'нет ответа' })
    Speed @('-x', 'http://127.0.0.1:7890')
    DisconnectTimed 'Системный прокси'
    $st = KlickJson status
    Check 'прокси: выключено' ($st.vpn -eq 'off') ("vpn=" + $st.vpn)
    $reg = ProxyReg
    Check 'системный прокси снят' ($reg.ProxyEnable -eq 0) ("ProxyEnable=" + $reg.ProxyEnable)
    $sys = IpSystemProxy
    Check 'после отключения программы с системным прокси ходят напрямую' (IsDirect $sys)

    # 4. VPN (TUN), всё через VPN
    KlickCli mode tun | Out-Null
    KlickCli connect | Out-Null
    $st = WaitVpn 'connected' 30
    Check 'TUN: подключено' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    Check 'адаптер klick поднят' ([bool](Get-NetAdapter -Name klick -ErrorAction SilentlyContinue | Where-Object Status -eq 'Up'))
    $vpn = IpVia
    Check 'адрес выхода по TCP — сервера' ([bool]$vpn -and -not (IsDirect $vpn))
    if ($directUdp) {
        $u = UdpIp
        Check 'UDP идёт через VPN (STUN видит сервер)' ([bool]$u -and (Net24 $u) -ne (Net24 $directUdp))
    }
    $rep = KlickJson ip
    if ($rep.via_vpn) { Log ("    {0}, {1}, {2}" -f $rep.via_vpn.country, $rep.via_vpn.city, $rep.via_vpn.provider) }
    Check 'DNS отвечает kl!ck' ($rep.dns_protected -eq $true)
    Check 'IPv6 не уходит мимо туннеля' ($rep.ipv6_leak -ne $true) ("ipv6_leak=" + $rep.ipv6_leak)
    Speed
    if ($fast.Count -gt 1) {
        $sw = KlickCli server $fast[1]
        Check "смена сервера на ходу: $($fast[1])" ($LASTEXITCODE -eq 0) $sw
        $st = WaitVpn 'connected' 20
        Check 'после смены сервера всё ещё подключено' ($st.vpn -eq 'connected')
        $ok = $false; $end = (Get-Date).AddSeconds(20)
        while (-not $ok -and (Get-Date) -lt $end) { $ok = [bool](IpVia); if (-not $ok) { Start-Sleep 1 } }
        Check 'после смены сервера страницы открываются' $ok
        KlickCli server $fast[0] | Out-Null
    }
    DisconnectTimed 'VPN (TUN)'
    Start-Sleep 1
    Check 'адаптер убран' (-not (Get-NetAdapter -Name klick -ErrorAction SilentlyContinue))
    $now = IpVia
    Check 'после отключения адрес снова свой' (IsDirect $now)

    # 4б. Включается и выключается быстро и каждый раз правильно: по 6 раз в каждом режиме
    foreach ($m in 'proxy', 'tun') {
        KlickCli mode $m | Out-Null
        $con = @(); $dis = @(); $bad = 0
        for ($i = 1; $i -le 6; $i++) {
            $t = Measure-Command { KlickCli connect | Out-Null; WaitVpn 'connected' 30 | Out-Null }
            $con += $t.TotalSeconds
            $st = KlickJson status
            $ip = if ($m -eq 'proxy') { IpVia 'curl.exe' @('-x', 'http://127.0.0.1:7890') } else { IpVia }
            if (-not $ip -or (IsDirect $ip)) { $bad++; Log "    $m, раз $i`: через VPN не пошло (vpn=$($st.vpn), адрес '$ip')" }
            $t = Measure-Command { KlickCli disconnect | Out-Null }
            $dis += $t.TotalSeconds
            $ip = IpVia
            if (-not (IsDirect $ip)) { $bad++; Log "    $m, раз $i`: после отключения адрес не свой ('$ip')" }
        }
        $c = $con | Measure-Object -Minimum -Maximum -Average
        $d = $dis | Measure-Object -Minimum -Maximum -Average
        Log ("    {0}: подключение {1:N1}…{2:N1} с (в среднем {3:N1}), отключение {4:N2}…{5:N2} с" -f $m, $c.Minimum, $c.Maximum, $c.Average, $d.Minimum, $d.Maximum)
        Check "$m`: 6 раз включился и выключился, трафик каждый раз где надо" ($bad -eq 0) "сбоев $bad"
        Check "$m`: подключение каждый раз быстрее 10 с" ($c.Maximum -le 10) ("{0:N1} с" -f $c.Maximum)
        Check "$m`: отключение каждый раз быстрее 2 с" ($d.Maximum -le 2) ("{0:N2} с" -f $d.Maximum)
    }

    # 4в. Смена режима на ходу
    KlickCli mode tun | Out-Null
    KlickCli connect | Out-Null
    WaitVpn 'connected' 30 | Out-Null
    KlickCli mode proxy | Out-Null
    $st = WaitVpn 'connected' 30
    $ip = IpVia 'curl.exe' @('-x', 'http://127.0.0.1:7890')
    $reg = ProxyReg
    Check 'TUN → прокси на ходу: подключено, прокси стоит, трафик через VPN' ($st.vpn -eq 'connected' -and $reg.ProxyEnable -eq 1 -and [bool]$ip -and -not (IsDirect $ip)) ("vpn={0}, ProxyEnable={1}" -f $st.vpn, $reg.ProxyEnable)
    Check 'TUN → прокси на ходу: адаптер убран' (-not (Get-NetAdapter -Name klick -ErrorAction SilentlyContinue | Where-Object Status -eq 'Up'))
    KlickCli mode tun | Out-Null
    $st = WaitVpn 'connected' 30
    Start-Sleep 1
    $ip = IpVia
    $reg = ProxyReg
    Check 'прокси → TUN на ходу: подключено, прокси снят, трафик через VPN' ($st.vpn -eq 'connected' -and $reg.ProxyEnable -eq 0 -and [bool]$ip -and -not (IsDirect $ip)) ("vpn={0}, ProxyEnable={1}" -f $st.vpn, $reg.ProxyEnable)

    # 4г. Стабильность: 3 минуты работы под нагрузкой (скачивание идёт фоном), запрос каждые 2 с
    $core0 = (CoreProc | Select-Object -First 1).ProcessId
    $svc = Get-Process klick-service -ErrorAction SilentlyContinue | Select-Object -First 1
    $svcStart = Mb $svc.Id
    $load = Start-Process curl.exe -ArgumentList '-s', '-o', 'NUL', '--max-time', '175', '--limit-rate', '3M', 'https://speed.cloudflare.com/__down?bytes=1000000000' -PassThru -WindowStyle Hidden
    $ok = 0; $bad = 0; $end = (Get-Date).AddMinutes(3)
    while ((Get-Date) -lt $end) {
        if ((Http) -eq '204') { $ok++ } else { $bad++; Log ("    {0:HH:mm:ss}: запрос не прошёл" -f (Get-Date)) }
        Start-Sleep 2
    }
    Stop-Process -Id $load.Id -Force -ErrorAction SilentlyContinue
    $st = KlickJson status
    $core1 = (CoreProc | Select-Object -First 1).ProcessId
    $svcEnd = Mb $svc.Id
    $coreMb = Mb $core1
    Log ("    запросов {0}, не прошло {1}; память: служба {2}→{3} МБ, ядро {4} МБ" -f ($ok + $bad), $bad, $svcStart, $svcEnd, $coreMb)
    Check 'стабильность: 3 минуты без обрывов (не прошло не больше 2% запросов)' ($ok -gt 0 -and $bad -le [math]::Max(1, [math]::Floor(($ok + $bad) * 0.02))) "не прошло $bad из $($ok + $bad)"
    Check 'стабильность: VPN всё время подключён, ядро не перезапускалось' ($st.vpn -eq 'connected' -and $core1 -and $core1 -eq $core0) ("vpn={0}, ядро {1}→{2}" -f $st.vpn, $core0, $core1)
    Check 'стабильность: служба лёгкая (меньше 100 МБ) и не растёт' ($svcEnd -gt 0 -and $svcEnd -lt 100 -and $svcEnd -le $svcStart + 20) ("{0}→{1} МБ" -f $svcStart, $svcEnd)
    Check 'стабильность: ядро меньше 400 МБ' ($coreMb -gt 0 -and $coreMb -lt 400) ("$coreMb МБ")
    KlickCli disconnect | Out-Null

    # 5. Сеть, где зашифрованный DNS (DoH Cloudflare и Яндекса) не работает: адреса серверов kl!ck
    #    должен найти через DNS системы.
    foreach ($proto in 'TCP', 'UDP') {
        New-NetFirewallRule -DisplayName "klick-test-doh-$proto" -Direction Outbound -Action Block -Protocol $proto -RemoteAddress 1.1.1.1, 1.0.0.1, 77.88.8.8, 77.88.8.1 -RemotePort 443, 853 | Out-Null
    }
    $doh = (Http 'curl.exe' 'https://1.1.1.1/dns-query') + '/' + (Http 'curl.exe' 'https://77.88.8.8/dns-query')
    Check 'DoH Cloudflare и Яндекса действительно недоступен' ($doh -eq '000/000') $doh
    KlickCli connect | Out-Null
    $st = WaitVpn 'connected' 30
    Check 'подключено без зашифрованного DNS' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $vpn = IpVia
    Check 'без зашифрованного DNS адрес выхода — сервера' ([bool]$vpn -and -not (IsDirect $vpn))
    KlickCli disconnect | Out-Null
    Get-NetFirewallRule -DisplayName 'klick-test-doh-*' -ErrorAction SilentlyContinue | Remove-NetFirewallRule

    # 6. Kill Switch с настоящим сервером, положение «только выбранное»
    New-Item -ItemType Directory -Force 'C:\kstest' | Out-Null
    Copy-Item "$env:WINDIR\System32\curl.exe" 'C:\kstest\curl.exe' -Force
    KlickCli routing selected | Out-Null
    Log ("ks add: " + ((KlickCli ks add 'C:\kstest') -replace '\s+', ' '))
    Start-Sleep 2
    Check 'VPN выключен — программа из Kill Switch без сети' ((Http 'C:\kstest\curl.exe') -eq '000')
    KlickCli connect | Out-Null
    $st = WaitVpn 'connected' 30
    Check 'Kill Switch: VPN подключён' ($st.vpn -eq 'connected')
    $ok = $false; $end = (Get-Date).AddSeconds(30)
    while (-not $ok -and (Get-Date) -lt $end) { $ok = KsViaVpn; if (-not $ok) { Start-Sleep 1 } }
    Check 'программа из Kill Switch ходит через VPN' $ok
    Check 'остальные ходят напрямую' (IsDirect (IpVia))
    $core = CoreProc | Select-Object -First 1
    if ($core) { Stop-Process -Id $core.ProcessId -Force }
    Check 'упало ядро — программа из Kill Switch не вышла напрямую' (KsNeverDirect 10)
    $ok = $false; $end = (Get-Date).AddSeconds(60)
    while (-not $ok -and (Get-Date) -lt $end) { $ok = KsViaVpn; if (-not $ok) { Start-Sleep 1 } }
    Check 'ядро вернулось, программа снова через VPN' $ok
    $svcProc = Get-Process klick-service -ErrorAction SilentlyContinue
    if ($svcProc) { Stop-Process -Id $svcProc.Id -Force }
    Check 'упала служба — программа из Kill Switch не вышла напрямую' (KsNeverDirect 10)
    $ok = $false; $end = (Get-Date).AddSeconds(60)
    while (-not $ok -and (Get-Date) -lt $end) { $ok = KsViaVpn; if (-not $ok) { Start-Sleep 1 } }
    Check 'служба вернулась, программа снова через VPN' $ok
    DisconnectTimed 'Kill Switch включён'
    Check 'VPN выключен — программа из Kill Switch не вышла напрямую' (KsNeverDirect 5)
    KlickCli ks rm 'C:\kstest' | Out-Null
    $ok = $false; $end = (Get-Date).AddSeconds(15)
    while (-not $ok -and (Get-Date) -lt $end) { $ok = (Http 'C:\kstest\curl.exe') -eq '204'; if (-not $ok) { Start-Sleep 1 } }
    Check 'убрана из Kill Switch — снова ходит напрямую' $ok

    # 7. Удаление вместе с данными (журнал службы — до удаления: с данными уйдёт и он)
    Copy-Item 'C:\ProgramData\klick\logs\service.log' "$out\service.log" -ErrorAction SilentlyContinue
    KlickCli mode proxy | Out-Null
    KlickCli connect | Out-Null
    WaitVpn 'connected' 30 | Out-Null
    $p = Start-Process "$root\klick-setup.exe" -ArgumentList '--silent', '--uninstall', '--wipe' -Wait -PassThru -WindowStyle Hidden
    Check 'удаление при включённом VPN: код 0' ($p.ExitCode -eq 0) "код $($p.ExitCode)"
    Check 'служба удалена' (-not (Get-Service klick -ErrorAction SilentlyContinue))
    Check 'ядро остановлено' (-not (Get-Process mihomo -ErrorAction SilentlyContinue))
    $reg = ProxyReg
    Check 'системного прокси kl!ck не осталось' (-not ($reg.ProxyEnable -eq 1 -and $reg.ProxyServer -eq '127.0.0.1:7890')) ("ProxyEnable={0}, ProxyServer={1}" -f $reg.ProxyEnable, $reg.ProxyServer)
    Check 'адаптера не осталось' (-not (Get-NetAdapter -Name klick -ErrorAction SilentlyContinue))
    Check 'интернет свой' (IsDirect (IpVia))
    Check 'программа из бывшего Kill Switch в сети' ((Http 'C:\kstest\curl.exe') -eq '204')
}
catch {
    $script:fails++
    Log "ОШИБКА СЦЕНАРИЯ: $($_.Exception.Message)"
}
finally {
    Get-NetFirewallRule -DisplayName 'klick-test-doh-*' -ErrorAction SilentlyContinue | Remove-NetFirewallRule
    Log ("ИТОГ: провалов {0}" -f $script:fails)
    Copy-Item 'C:\ProgramData\klick\logs\service.log' "$out\service.log" -ErrorAction SilentlyContinue
    Set-Content "$out\real-done.txt" 'done'
    if (-not $NoShutdown) {
        Start-Sleep 2
        shutdown.exe /s /t 0
    }
}
