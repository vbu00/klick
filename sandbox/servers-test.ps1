# Серверы подписки под лупой — на раннере CI (sandbox\ci.ps1) или тестовом компьютере. Ссылка — в
# переменной KLICK_TEST_SUB; в отчёт идут только имена серверов, задержки, коды и состояния, адреса
# серверов в строках ядра заменяются на <адрес>.
#   1. Голое ядро mihomo с теми же серверами: замер, как у kl!ck (группа, 5 с, unified-delay), и по
#      одному серверу.
#   2. Голое ядро под нагрузкой на самом быстром wireguard и hysteria2: просто порт, без remote-dns-resolve,
#      с DNS и сниффером как у kl!ck, TUN как у kl!ck. Так видно, чья беда — протокола, ядра или настроек kl!ck.
#   3. kl!ck: задержка при выключенном VPN; VPN (TUN) на тех же двух серверах — открываются ли страницы,
#      что делает страж связи (переподключения, «Сервер не отвечает», перезапуски ядра), задержка при
#      включённом VPN.
#   4. «Только выбранное» и свой сайт colab.research.google.com: какие соединения страницы Colab
#      идут через VPN, а какие напрямую.
# Меняет настройки сети — только на тестовой машине.
param([switch]$NoShutdown)

$ErrorActionPreference = 'Continue'
# Имена серверов с флагами и кириллица из klick-cli и ядра — в UTF-8.
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$root = 'C:\klick'
$out = Join-Path $root 'results'
$inst = 'C:\Program Files\klick'
$svcLog = 'C:\ProgramData\klick\logs\service.log'
New-Item -ItemType Directory -Force $out | Out-Null
$report = Join-Path $out 'servers-report.txt'
Set-Content -Path $report -Value '' -Encoding UTF8
$script:fails = 0
$sub = $env:KLICK_TEST_SUB
# Сколько держать VPN на каждом сервере: страж проверяет связь раз в 30 с.
$hold = if ($env:KLICK_HOLD_SECONDS) { [int]$env:KLICK_HOLD_SECONDS } else { 90 }

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
function Http([string]$url = 'https://www.gstatic.com/generate_204', [int]$timeout = 10) {
    $code = & curl.exe -s -o NUL -w '%{http_code}' --max-time $timeout $url 2>$null
    if ($code) { "$code".Trim() } else { '000' }
}
# Адреса и домены из строк ядра — прочь: в отчёте только имена серверов и суть ошибки. Адреса
# проверки (gstatic, Cloudflare, Google) оставляем — по ним видно, что именно не открылось.
function Scrub([string]$line) {
    $line = $line -replace '\b\d{1,3}(\.\d{1,3}){3}(:\d+)?\b', '<адрес>'
    $line = $line -replace '\[[0-9a-fA-F:]{3,}\](:\d+)?', '<адрес>'
    [regex]::Replace($line, '\b[A-Za-z0-9-]+(\.[A-Za-z0-9-]+)*\.[A-Za-z]{2,}(:\d+)?\b', {
        param($m)
        if ($m.Value -match '(^|\.)(gstatic\.com|cloudflare\.com|google\.com|googleapis\.com)(:\d+)?$') { $m.Value } else { '<адрес>' }
    })
}
function CoreProc { Get-CimInstance Win32_Process -Filter "Name='mihomo.exe'" | Where-Object { $_.CommandLine -like '*ProgramData*' -and $_.CommandLine -like '*config.yaml*' -and $_.CommandLine -notlike '*guard*' } }
function Latency([string]$what) {
    $t = Measure-Command { $script:lat = @(KlickJson latency) }
    $ok = @($script:lat | Where-Object { $_.delay -gt 0 }).Count
    Log ("    задержка ({0}, {1:N1} с): ответили {2} из {3}" -f $what, $t.TotalSeconds, $ok, $script:lat.Count)
    foreach ($s in $script:lat) { Log ("      {0,-26} {1,-10} {2}" -f $s.name, $s.kind, $(if ($s.delay) { "$($s.delay) мс" } else { 'нет ответа' })) }
    $script:lat
}

# ── Голое ядро ───────────────────────────────────────────────────────────────
$mihomo = "$root\resources\core\mihomo.exe"
$bare = 'C:\bare-core'
$bareUrl = 'http://127.0.0.1:19190'
$bareHead = @{ Authorization = 'Bearer diag' }
# Ответ ядра — UTF-8 без charset в заголовке: Windows PowerShell сам прочитал бы его как Latin-1.
function BareJson([string]$u, [int]$timeoutSec = 10) {
    $r = Invoke-WebRequest -UseBasicParsing -Headers $bareHead $u -TimeoutSec $timeoutSec
    [Text.Encoding]::UTF8.GetString($r.RawContentStream.ToArray()) | ConvertFrom-Json
}
# Варианты голого ядра: $klickDns — DNS и сниффер как у kl!ck (имена сайтов спрашиваются через
# сервер), $tun — перехват всей системы, как в kl!ck; $servers — файл серверов.
function BareConfig([bool]$unified = $true, [string]$level = 'warning', [bool]$klickDns = $false, [bool]$tun = $false, [string]$servers = 'servers.yaml') {
    $ns = if ($klickDns) { '["https://1.1.1.1/dns-query#klick-vpn", "https://8.8.8.8/dns-query#klick-vpn"]' } else { '["tls://77.88.8.8:853", "77.88.8.8"]' }
    $sniffer = if (-not $klickDns) { '' } else { @"
sniffer:
  enable: true
  force-dns-mapping: true
  parse-pure-ip: true
  override-destination: false
  sniff:
    TLS: { ports: [443, 8443], override-destination: true }
    HTTP: { ports: [80, "8080-8880"], override-destination: true }
    QUIC: { ports: [443, 8443], override-destination: true }
"@ }
    $tunBlock = if (-not $tun) { '' } else { @"
tun:
  enable: true
  stack: gvisor
  device: bare
  auto-route: true
  auto-detect-interface: true
  strict-route: true
  dns-hijack: ["any:53", "tcp://any:53"]
"@ }
@"
mode: rule
log-level: $level
ipv6: true
unified-delay: $(if ($unified) { 'true' } else { 'false' })
tcp-concurrent: true
find-process-mode: always
geo-auto-update: false
geodata-mode: false
external-controller: 127.0.0.1:19190
secret: diag
mixed-port: 19180
profile: { store-selected: false, store-fake-ip: false }
proxy-providers:
  klick-servers: { type: file, path: $servers, health-check: { enable: false } }
proxy-groups:
  - { name: klick-vpn, type: select, use: [klick-servers] }
rules:
  - IP-CIDR,127.0.0.0/8,DIRECT,no-resolve
  - IP-CIDR,10.0.0.0/8,DIRECT,no-resolve
  - IP-CIDR,168.63.129.16/32,DIRECT,no-resolve
  - IP-CIDR,169.254.0.0/16,DIRECT,no-resolve
  - MATCH,klick-vpn
dns:
  enable: true
  ipv6: false
  enhanced-mode: fake-ip
  fake-ip-range: 198.18.0.1/16
  default-nameserver: [77.88.8.8, 1.1.1.1]
  nameserver: $ns
  direct-nameserver: ["tls://77.88.8.8:853", "77.88.8.8"]
  proxy-server-nameserver: ["tls://77.88.8.8:853", "https://1.1.1.1/dns-query", "77.88.8.8"]
$sniffer
$tunBlock
"@
}
function BareStart([bool]$unified = $true, [string]$level = 'warning', [bool]$klickDns = $false, [bool]$tun = $false, [string]$servers = 'servers.yaml') {
    BareStop
    Set-Content "$bare\config.yaml" (BareConfig $unified $level $klickDns $tun $servers) -Encoding UTF8
    $script:bareProc = Start-Process $mihomo -ArgumentList '-d', $bare, '-f', "$bare\config.yaml" -PassThru -WindowStyle Hidden `
        -RedirectStandardOutput "$bare\core.out" -RedirectStandardError "$bare\core.err"
    $end = (Get-Date).AddSeconds(15)
    while ((Get-Date) -lt $end) {
        try { BareJson "$bareUrl/version" 2 | Out-Null; Start-Sleep 2; return $true } catch { Start-Sleep -Milliseconds 300 }
    }
    $false
}
function BareStop { if ($script:bareProc) { Stop-Process -Id $script:bareProc.Id -Force -ErrorAction SilentlyContinue; $script:bareProc = $null; Start-Sleep 1 } }
function BareGroup([string]$url, [int]$timeout) {
    $u = "$bareUrl/group/klick-vpn/delay?url=$([uri]::EscapeDataString($url))&timeout=$timeout"
    $t = [Diagnostics.Stopwatch]::StartNew()
    try { $r = BareJson $u ([math]::Ceiling($timeout / 1000) + 10) } catch { $r = $null }
    $t.Stop()
    $names = if ($r) { @($r.PSObject.Properties | ForEach-Object { '{0}: {1} мс' -f $_.Name, $_.Value }) } else { @() }
    Log ("      ответили {0} за {1:N1} с: {2}" -f $names.Count, $t.Elapsed.TotalSeconds, $(if ($names) { $names -join ' · ' } else { '—' }))
    $names.Count
}
function BareOne([string]$name, [string]$url, [int]$timeout) {
    $u = "$bareUrl/providers/proxies/klick-servers/$([uri]::EscapeDataString($name))/healthcheck?url=$([uri]::EscapeDataString($url))&timeout=$timeout"
    try { $r = BareJson $u ([math]::Ceiling($timeout / 1000) + 10); "$($r.delay) мс" } catch { 'нет ответа' }
}
function BareWarnings {
    $lines = Get-Content "$bare\core.out", "$bare\core.err" -Encoding UTF8 -ErrorAction SilentlyContinue | Where-Object { $_ -match 'level=(warning|error)' }
    $uniq = @($lines | ForEach-Object { Scrub (($_ -replace '^.*msg="', '') -replace '"\s*$', '') } | Group-Object | Sort-Object Count -Descending | Select-Object -First 8)
    foreach ($g in $uniq) { Log ("      ядро ({0}×): {1}" -f $g.Count, $g.Name) }
}

# Голое ядро под нагрузкой: запрос каждые 2 с и проверка, как у стража kl!ck, раз в 30 с; по строкам
# WireGuard видно, когда туннель замолкает («не слышно сервер») и сколько было рукопожатий.
function BareLoad([string]$label, [string]$server, [int]$seconds, [bool]$viaTun = $false) {
    # С TUN ядро дочитывает серверы дольше: первые попытки выбрать сервер получают 400.
    $body = [Text.Encoding]::UTF8.GetBytes((@{ name = $server } | ConvertTo-Json -Compress))
    $why = ''
    for ($i = 0; $i -lt 10; $i++) {
        try { Invoke-WebRequest -UseBasicParsing -Method Put -Headers $bareHead -ContentType 'application/json' -Body $body "$bareUrl/proxies/klick-vpn" -TimeoutSec 5 | Out-Null; $why = ''; break }
        catch {
            $why = $_.Exception.Message
            try { $why += ' ' + (New-Object IO.StreamReader($_.Exception.Response.GetResponseStream())).ReadToEnd() } catch {}
            Start-Sleep 1
        }
    }
    if ($why) { Log ("    {0}: сервер не выбран: {1}" -f $label, (Scrub $why)); return }
    $via = if ($viaTun) { @() } else { @('-x', 'http://127.0.0.1:19180') }
    $ok = 0; $bad = 0; $pOk = 0; $pBad = 0; $slow = 0
    $probe = "$bareUrl/proxies/klick-vpn/delay?url=$([uri]::EscapeDataString('https://www.gstatic.com/generate_204'))&timeout=5000"
    $end = (Get-Date).AddSeconds($seconds)
    $nextProbe = (Get-Date).AddSeconds(5)
    while ((Get-Date) -lt $end) {
        $r = & curl.exe -s -o NUL -w '%{http_code} %{time_total}' --max-time 8 @via 'https://www.gstatic.com/generate_204' 2>$null
        $code, $time = "$r".Trim() -split ' '
        if ($code -eq '204') { $ok++; if ([double]::Parse($time, [Globalization.CultureInfo]::InvariantCulture) -gt 2) { $slow++ } } else { $bad++ }
        if ((Get-Date) -ge $nextProbe) {
            try { BareJson $probe 10 | Out-Null; $pOk++ } catch { $pBad++ }
            $nextProbe = (Get-Date).AddSeconds(30)
        }
        Start-Sleep 2
    }
    $log = Get-Content "$bare\core.out" -Encoding UTF8 -ErrorAction SilentlyContinue
    $n = { param($re) @($log | Where-Object { $_ -match $re }).Count }
    Log ("    {0}: запросы {1} из {2} (дольше 2 с: {3}); проверки, как у стража: {4} из {5}; WireGuard: «не слышно сервер» {6}, рукопожатий {7}, без ответа {8}" -f `
        $label, $ok, ($ok + $bad), $slow, $pOk, ($pOk + $pBad), (& $n 'stopped hearing back'), (& $n 'Sending handshake initiation'), (& $n 'Handshake did not complete'))
    @{ ok = $ok; total = ($ok + $bad); probes = $pOk; ptotal = ($pOk + $pBad) }
}

# ── VPN на одном сервере несколько минут ─────────────────────────────────────
function Hold([string]$server, [int]$seconds) {
    $sel = KlickCli server $server
    Log "--- TUN на «$server» $seconds с"
    if ($LASTEXITCODE -ne 0) { Log ("    сервер не выбран: " + (Scrub $sel)) }
    $st = KlickJson status
    if ($st.vpn -ne 'connected') { KlickCli connect | Out-Null; $st = WaitVpn 'connected' 30 }
    Check "подключено на «$server»" ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $logStart = @(Get-Content $svcLog -Encoding UTF8 -ErrorAction SilentlyContinue).Count
    $watchFile = "$out\watch-$([guid]::NewGuid().ToString('N')).txt"
    $watch = Start-Process "$inst\klick-cli.exe" -ArgumentList '--prod', 'watch' -PassThru -WindowStyle Hidden -RedirectStandardOutput $watchFile -RedirectStandardError "$watchFile.err"
    $core0 = (CoreProc | Select-Object -First 1).ProcessId
    $pages = 0; $failed = 0; $states = @(); $last = 'connected'; $cores = @($core0)
    $end = (Get-Date).AddSeconds($seconds)
    $nextHttp = Get-Date
    while ((Get-Date) -lt $end) {
        $s = KlickJson status
        $v = if ($s) { $s.vpn } else { '?' }
        if ($v -ne $last) {
            $att = if ($s.attempt) { " ($($s.attempt[0]) из $($s.attempt[1]))" } else { '' }
            $states += ('{0:HH:mm:ss} {1}{2}' -f (Get-Date), $v, $att)
            $last = $v
        }
        $pid1 = (CoreProc | Select-Object -First 1).ProcessId
        if ($pid1 -and $pid1 -ne $cores[-1]) { $cores += $pid1 }
        if ((Get-Date) -ge $nextHttp) {
            if ((Http) -eq '204') { $pages++ } else { $failed++ }
            $nextHttp = (Get-Date).AddSeconds(5)
        }
        Start-Sleep -Milliseconds 700
    }
    Stop-Process -Id $watch.Id -Force -ErrorAction SilentlyContinue
    $events = Get-Content $watchFile -Encoding UTF8 -ErrorAction SilentlyContinue
    $count = { param($re) @($events | Where-Object { $_ -match $re }).Count }
    $down = & $count '"vpn\.down"'
    $restored = & $count '"vpn\.restored"'
    $switched = & $count '"server\.switched"'
    Log ("    страницы: открылись {0}, не открылись {1}" -f $pages, $failed)
    Log ("    состояния: {0}" -f $(if ($states) { $states -join ' → ' } else { 'всё время connected' }))
    Log ("    уведомления: «Сервер не отвечает» {0}, «Связь восстановлена» {1}, смена сервера {2}; ядро запускалось {3} раз" -f $down, $restored, $switched, ($cores.Count - 1))
    $newLog = @(Get-Content $svcLog -Encoding UTF8 -ErrorAction SilentlyContinue | Select-Object -Skip $logStart)
    foreach ($l in ($newLog | Where-Object { $_ -match 'WARN|ERROR' } | Select-Object -Last 12)) { Log ("    журнал: " + (Scrub $l)) }
    Latency "VPN включён, «$server»" | Out-Null
    Check "«$server»: страницы открываются" ($pages -gt 0 -and $failed -le [math]::Max(1, [math]::Floor(($pages + $failed) * 0.05))) "не открылись $failed из $($pages + $failed)"
    Check "«$server»: страж не терял связь (без переподключений и перезапусков ядра)" ($states.Count -eq 0 -and $cores.Count -eq 1 -and $down -eq 0) ("переходов {0}, перезапусков ядра {1}, «не отвечает» {2}" -f $states.Count, ($cores.Count - 1), $down)
}

try {
    if (-not $sub) { throw 'нет KLICK_TEST_SUB' }
    Log ("Windows {0}, {1}" -f [Environment]::OSVersion.Version, (Get-CimInstance Win32_OperatingSystem).Caption)
    Check 'интернет напрямую есть' ((Http) -eq '204')

    # Голое ядро — до установки kl!ck: ни TUN, ни служба не мешают.
    New-Item -ItemType Directory -Force $bare | Out-Null
    & curl.exe -s -f -A 'mihomo/1.19.31' --max-time 30 -o "$bare\servers.yaml" $sub 2>$null
    Check 'голое ядро: подписка скачана' ((Test-Path "$bare\servers.yaml") -and (Get-Item "$bare\servers.yaml").Length -gt 0)
    # Те же серверы, но адреса сайтов WireGuard ищет у себя, а не через туннель.
    $local = (Get-Content "$bare\servers.yaml" -Raw -Encoding UTF8) -replace 'remote-dns-resolve:\s*true', 'remote-dns-resolve: false'
    [IO.File]::WriteAllText("$bare\servers-local-dns.yaml", $local, (New-Object Text.UTF8Encoding $false))
    $script:fastest = @{}
    if (BareStart) {
        Log ("--- голое ядро {0}" -f (BareJson "$bareUrl/version").version)
        Log '    группа, gstatic, 5 с (как kl!ck):'
        $asKlick = BareGroup 'https://www.gstatic.com/generate_204' 5000
        Log '    группа, второй раз:'
        $asKlick2 = BareGroup 'https://www.gstatic.com/generate_204' 5000
        Log '    по одному, gstatic, 10 с:'
        $prov = BareJson "$bareUrl/providers/proxies/klick-servers"
        foreach ($p in $prov.proxies) {
            $d = BareOne $p.name 'https://www.gstatic.com/generate_204' 10000
            Log ("      {0,-26} {1,-10} {2}" -f $p.name, $p.type, $d)
            if ($d -match '^(\d+) мс$') {
                $ms = [int]$Matches[1]; $k = "$($p.type)".ToLower()
                if (-not $script:fastest[$k] -or $ms -lt $script:fastest[$k].ms) { $script:fastest[$k] = @{ name = $p.name; ms = $ms } }
            }
        }
        BareWarnings
        Check 'голое ядро: задержка измерена хотя бы у одного (как kl!ck)' (($asKlick + $asKlick2) -gt 0)
    } else { Check 'голое ядро запустилось' $false }
    $hy = if ($script:fastest['hysteria2']) { $script:fastest['hysteria2'].name } else { $null }
    $wg = if ($script:fastest['wireguard']) { $script:fastest['wireguard'].name } else { $null }
    Log ("--- голое ядро под нагрузкой, {0} с на вариант: wireguard «{1}», hysteria2 «{2}»" -f $hold, $wg, $hy)
    $variants = @(
        @{ label = 'hysteria2, порт'; server = $hy; klickDns = $false; tun = $false; servers = 'servers.yaml' },
        @{ label = 'wireguard, порт'; server = $wg; klickDns = $false; tun = $false; servers = 'servers.yaml' },
        @{ label = 'wireguard, порт, remote-dns-resolve: false'; server = $wg; klickDns = $false; tun = $false; servers = 'servers-local-dns.yaml' },
        @{ label = 'wireguard, порт, DNS и сниффер как у kl!ck'; server = $wg; klickDns = $true; tun = $false; servers = 'servers.yaml' },
        @{ label = 'wireguard, TUN как у kl!ck'; server = $wg; klickDns = $true; tun = $true; servers = 'servers.yaml' },
        @{ label = 'wireguard, TUN как у kl!ck, remote-dns-resolve: false'; server = $wg; klickDns = $true; tun = $true; servers = 'servers-local-dns.yaml' },
        @{ label = 'hysteria2, TUN как у kl!ck'; server = $hy; klickDns = $true; tun = $true; servers = 'servers.yaml' }
    )
    foreach ($v in $variants) {
        if (-not $v.server) { continue }
        if (BareStart $true 'debug' $v.klickDns $v.tun $v.servers) { BareLoad $v.label $v.server $hold $v.tun | Out-Null }
        else { Log ("    {0}: ядро не запустилось" -f $v.label); BareWarnings }
    }
    BareStop
    Remove-Item $bare -Recurse -Force -ErrorAction SilentlyContinue

    # kl!ck
    $p = Start-Process "$root\klick-setup.exe" -ArgumentList '--silent', '--no-autostart' -Wait -PassThru -WindowStyle Hidden
    Check 'установка: код 0' ($p.ExitCode -eq 0) "код $($p.ExitCode)"
    Copy-Item "$root\bin\klick-cli.exe" $inst -Force
    $end = (Get-Date).AddSeconds(20)
    while (-not (KlickJson status) -and (Get-Date) -lt $end) { Start-Sleep 1 }
    $about = KlickJson about
    Log ("kl!ck {0}, ядро {1}" -f $about.version, $about.core_version)
    $added = KlickCli add $sub
    Check 'подписка добавлена' ($LASTEXITCODE -eq 0 -and $added -notmatch 'error|ошибк') (($added -replace [regex]::Escape($sub), '<ссылка>') -split "`n" | Select-Object -First 1)

    $servers = @(KlickJson servers)
    Log ("    серверов: {0} · {1}" -f $servers.Count, ((@($servers | ForEach-Object { $_.kind }) | Sort-Object -Unique) -join ', '))
    $off1 = Latency 'VPN выключен'
    Check 'VPN выключен: задержка измерена хотя бы у одного' (@($off1 | Where-Object { $_.delay -gt 0 }).Count -gt 0)
    $pick = @(@($hy, $wg) | Where-Object { $_ })
    if (-not $pick) { $pick = @($servers[0].name) }
    Log ("    VPN проверю на: {0}" -f ($pick -join ' · '))

    KlickCli mode tun | Out-Null
    KlickCli routing all | Out-Null
    foreach ($name in $pick) { Hold $name $hold }

    # «Только выбранное» + свой сайт: куда идут соединения страницы Colab.
    Log '--- «Только выбранное», свой сайт colab.research.google.com, TUN'
    $best = $pick[0]
    KlickCli server $best | Out-Null
    KlickCli routing selected | Out-Null
    Log ("    список: " + ((KlickCli list add selected domain colab.research.google.com vpn) -replace '\s+', ' '))
    $st = WaitVpn 'connected' 20
    Check '«Только выбранное»: подключено' ($st.vpn -eq 'connected') ("vpn=" + $st.vpn)
    $colab = & curl.exe -s -o NUL -w '%{http_code} %{remote_ip}' --max-time 15 'https://colab.research.google.com/' 2>$null
    Log "    curl colab.research.google.com: $colab"
    $edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($edge) {
        $prof = 'C:\edge-colab'
        $e = Start-Process $edge -ArgumentList '--headless=new', '--disable-gpu', '--no-first-run', "--user-data-dir=$prof", '--remote-debugging-port=9333', 'https://colab.research.google.com/' -PassThru
        $seen = @{}
        $end = (Get-Date).AddSeconds(30)
        while ((Get-Date) -lt $end) {
            foreach ($c in @(KlickJson conns)) { if ($c.host) { $seen["$($c.host)|$($c.route)|$($c.network)"] = $c.process } }
            Start-Sleep 2
        }
        Get-Process msedge -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $edge } | Stop-Process -Force -ErrorAction SilentlyContinue
        $rows = @($seen.Keys | Where-Object { $_ -match 'google|gstatic|colab|googleusercontent|googleapis|ggpht|youtube' } | Sort-Object)
        Log ("    соединения Edge к Google за 30 с: {0}" -f $rows.Count)
        foreach ($r in $rows) { $h, $route, $net = $r -split '\|'; Log ("      {0,-6} {1,-4} {2}" -f $route, $net, $h) }
        $direct = @($rows | Where-Object { ($_ -split '\|')[1] -eq 'direct' -and ($_ -split '\|')[0] -match 'colab' })
        Check 'страница Colab: соединения к colab.* идут через VPN' ($rows.Count -gt 0 -and $direct.Count -eq 0) ("напрямую: {0}" -f $(if ($direct) { ($direct | ForEach-Object { ($_ -split '\|')[0] }) -join ', ' } else { 'нет' }))
        foreach ($f in @(KlickJson failures | Select-Object -First 10)) { if ($f.host -match 'google|colab|gstatic') { Log ("    не открылось: {0} ({1}): {2}" -f $f.host, $f.route, (Scrub $f.error)) } }
        Remove-Item $prof -Recurse -Force -ErrorAction SilentlyContinue
    } else { Log '    Edge не найден — страницу Colab не открываю' }
    KlickCli disconnect | Out-Null
}
catch {
    Log "ОШИБКА СЦЕНАРИЯ: $($_.Exception.Message -replace [regex]::Escape("$sub"), '<ссылка>')"
    $script:fails++
}
finally {
    BareStop
    Get-Process klick -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
    if (Test-Path "$inst\klick-setup.exe") { Start-Process "$inst\klick-setup.exe" -ArgumentList '--silent', '--uninstall', '--wipe' -Wait -WindowStyle Hidden }
    Remove-Item "$out\watch-*.txt*" -ErrorAction SilentlyContinue
    Log "ИТОГ: провалов $script:fails"
    Set-Content "$out\script-done.txt" 'done'
    if (-not $NoShutdown) { Start-Sleep 2; shutdown.exe /s /t 0 }
}
