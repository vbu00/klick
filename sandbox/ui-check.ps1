# Установщик, окно и трей глазами человека — в Песочнице Windows.
#   powershell -File sandbox\start.ps1 -Release -Script ui-check
# 1. «Прежняя kl!ck 0.2–0.4»: её зашифрованные подписки в %LOCALAPPDATA%\com.vbu00.klick.
# 2. klick-setup.exe --silent: ставит WebView2, если его нет, и переносит подписки.
# 3. Окно и трей: снимки всего экрана и самих страниц (протокол отладки WebView2); экраны и листы
#    нового окна — щелчками по кнопкам; новые команды службы — по каналу управления.
# 4. Окно не разворачивается: ни командой «Развернуть», ни Win+↑.
# 5. Канал управления занят чужой программой — окно не отдаёт ей ни одной команды.
# Отчёт — results\ui-check.txt, снимки — results\*.png.

$ErrorActionPreference = 'Continue'
$root = 'C:\klick'
$out = Join-Path $root 'results'
$inst = 'C:\Program Files\klick'
New-Item -ItemType Directory -Force $out | Out-Null
$report = Join-Path $out 'ui-check.txt'
Set-Content -Path $report -Value '' -Encoding UTF8
$script:fails = 0
function Log([string]$m) {
    $line = "[{0:HH:mm:ss}] {1}" -f (Get-Date), $m
    for ($i = 0; $i -lt 20; $i++) { try { Add-Content -Path $report -Value $line -Encoding UTF8 -ErrorAction Stop; return } catch { Start-Sleep -Milliseconds 250 } }
}
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if (-not $ok) { $script:fails++ }
    $mark = if ($ok) { 'OK  ' } else { 'FAIL' }
    $tail = if ($detail) { " - $detail" } else { '' }
    Log "$mark $name$tail"
}
function KlickJson { $t = & "$inst\klick-cli.exe" --prod @args 2>$null | Out-String; try { $t | ConvertFrom-Json | ForEach-Object { $_ } } catch { $null } }
function WebView2Installed {
    foreach ($k in 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}',
                   'HKCU:\Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}') {
        $pv = (Get-ItemProperty $k -ErrorAction SilentlyContinue).pv
        if ($pv -and $pv -ne '0.0.0.0') { return $true }
    }
    return $false
}

Add-Type -AssemblyName System.Windows.Forms, System.Drawing, System.Security, UIAutomationClient, UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class W {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, int msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, int flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern int GetDpiForWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(int flags, int dx, int dy, int data, IntPtr extra);
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern int GetWindowThreadProcessId(IntPtr h, out int pid);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
}
'@
function Shot([string]$name) {
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save((Join-Path $out "$name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}
# Видимые окна процесса: размер и место.
function WindowsOf([int]$procId) {
    $list = New-Object System.Collections.ArrayList
    $cb = [W+EnumProc]{ param($h, $l) $p = 0; [void][W]::GetWindowThreadProcessId($h, [ref]$p); if ($p -eq $procId -and [W]::IsWindowVisible($h)) { [void]$list.Add((RectOf $h)) }; $true }
    [void][W]::EnumWindows($cb, [IntPtr]::Zero)
    $list -join '; '
}
# Значок kl!ck — на панель задач (Windows 11 прячет новые значки за стрелкой), потом щелчок по нему:
# правый открывает окно трея, левый — главное окно.
function ClickTrayIcon([switch]$Right) {
    Get-ChildItem 'HKCU:\Control Panel\NotifyIconSettings' -ErrorAction SilentlyContinue | Where-Object { (Get-ItemProperty $_.PSPath).ExecutablePath -like '*klick.exe' } |
        ForEach-Object { Set-ItemProperty $_.PSPath -Name IsPromoted -Value 1 -Type DWord }
    Start-Sleep 3
    $ae = [System.Windows.Automation.AutomationElement]
    $tray = $ae::RootElement.FindFirst([System.Windows.Automation.TreeScope]::Children, (New-Object System.Windows.Automation.PropertyCondition($ae::ClassNameProperty, 'Shell_TrayWnd')))
    if (-not $tray) { return 'нет панели задач' }
    # Значок в трее — SystemTray.NormalButton; кнопка окна на панели задач тоже зовётся «kl!ck»,
    # щелчок по ней сворачивает окно, а трей не открывает.
    $btn = $tray.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
        Where-Object { $_.Current.ClassName -eq 'SystemTray.NormalButton' -and $_.Current.Name -match 'kl!ck' } | Select-Object -First 1
    if (-not $btn) { return 'значок kl!ck в трее не найден' }
    $r = $btn.Current.BoundingRectangle
    $x = [int]($r.X + $r.Width / 2); $y = [int]($r.Y + $r.Height / 2)
    [void][W]::SetCursorPos($x, $y); Start-Sleep -Milliseconds 300
    # MOUSEEVENTF_LEFTDOWN/UP = 2/4, RIGHTDOWN/UP = 8/16
    $down, $up, $which = if ($Right) { 8, 16, 'правой' } else { 2, 4, 'левой' }
    [W]::mouse_event($down, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 80; [W]::mouse_event($up, 0, 0, 0, [IntPtr]::Zero)
    "щёлкнул $which «$($btn.Current.Name)» в $x,$y"
}
function RectOf([IntPtr]$h) { $r = New-Object W+RECT; [void][W]::GetWindowRect($h, [ref]$r); "{0}x{1} @ {2},{3}" -f ($r.R - $r.L), ($r.B - $r.T), $r.L, $r.T }

try {
    Log ("Песочница: Windows {0}, экран {1}, WebView2 до установки: {2}" -f [Environment]::OSVersion.Version, [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Size, (WebView2Installed))

    # 1. Прежняя kl!ck: только данные — как после неудачного удаления или у друга с 0.3/0.4.
    $oldData = Join-Path $env:LOCALAPPDATA 'com.vbu00.klick'
    New-Item -ItemType Directory -Force $oldData | Out-Null
    $vault = @'
{"profiles":[
 {"id":"a","kind":"single","name":"Старый сервер","url":"vless://1b2c3d4e-0000-4000-8000-000000000001@203.0.113.5:443?type=tcp&security=reality&pbk=Q2sDgDCjyNy_9Ydp1s7yI3i7WEKtMP3vOIZ2Ahj1OxM&fp=chrome&sni=www.vk.ru&sid=ab12#Old","proxies":[{"name":"Old"}]},
 {"id":"b","kind":"file","name":"Файл","proxies":[{"name":"DE","type":"ss","server":"198.51.100.7","port":443,"cipher":"aes-128-gcm","password":"x"}]}
]}
'@
    $bytes = [Text.Encoding]::UTF8.GetBytes($vault)
    $enc = [Security.Cryptography.ProtectedData]::Protect($bytes, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    [IO.File]::WriteAllBytes((Join-Path $oldData 'profiles.dat'), $enc)
    Set-Content (Join-Path $oldData 'settings.json') '{"mode":"tun"}' -Encoding UTF8
    Log 'прежняя kl!ck: 2 подключения в profiles.dat'

    # 2. Установщик без окна. WebView2 — заранее из полного установщика, если start.ps1 его положил:
    # загрузчик, которым его ставит klick-setup, в Песочнице качает до четверти часа.
    $wv = Join-Path $root 'webview2.exe'
    if ((Test-Path $wv) -and -not (WebView2Installed)) {
        $sig = Get-AuthenticodeSignature $wv
        if ($sig.Status -eq 'Valid' -and $sig.SignerCertificate.Subject -match 'O=Microsoft Corporation') {
            $t0 = Get-Date
            # WaitForExit, а не -Wait: -Wait ждёт и фоновые процессы Edge Update, которые остаются жить.
            [void](Start-Process $wv -ArgumentList '/silent', '/install' -PassThru).WaitForExit(600000)
            Log ("WebView2 из полного установщика за {0:N0} с: {1}" -f ((Get-Date) - $t0).TotalSeconds, (WebView2Installed))
        } else { Log 'webview2.exe: подпись не Microsoft — не запускаю' }
    }
    $t0 = Get-Date
    $p = Start-Process "$root\klick-setup.exe" -ArgumentList '--silent', '--no-autostart' -Wait -PassThru -WindowStyle Hidden -RedirectStandardOutput "$out\setup.out" -RedirectStandardError "$out\setup.err"
    $setupOut = (Get-Content "$out\setup.out", "$out\setup.err" -Encoding UTF8 -ErrorAction SilentlyContinue) -join ' | '
    Log ("установщик: код {0} за {1:N0} с: {2}" -f $p.ExitCode, ((Get-Date) - $t0).TotalSeconds, $setupOut)
    Check 'установщик отработал' ($p.ExitCode -eq 0)
    Check 'WebView2 есть после установки' (WebView2Installed)
    Copy-Item "$root\bin\klick-cli.exe" $inst -Force
    $names = @((KlickJson settings).connections | ForEach-Object { $_.name })
    Check 'подписки прежней kl!ck перенесены' ($names.Count -eq 2) ($names -join ', ')
    Check 'данные прежней kl!ck убраны' (-not (Test-Path (Join-Path $oldData 'profiles.dat')))

    # 3. Окно и трей
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9341'
    $win = Start-Process "$inst\klick.exe" -PassThru
    Start-Sleep 8
    Check 'окно kl!ck работает' (-not $win.HasExited)
    $win.Refresh()
    $h = $win.MainWindowHandle
    Log ("главное окно: {0}, DPI {1}" -f (RectOf $h), [W]::GetDpiForWindow($h))
    Shot 'desk-main'
    Log ("окна kl!ck: " + (WindowsOf $win.Id))
    Log ("трей: " + (ClickTrayIcon -Right))
    Start-Sleep 2
    $after = WindowsOf $win.Id
    Log ("окна kl!ck после щелчка: " + $after)
    # Новое окно шириной около 320 (с тенью — больше) — окно трея; системного меню быть не должно.
    $trayWin = @($after -split '; ' | Where-Object { $_ -match '^(3[2-4]\d)x(\d+) ' -and [int]$Matches[2] -gt 100 })
    Check 'правый щелчок по значку открывает окно трея' ($trayWin.Count -eq 1) $after
    Shot 'desk-tray'
    # Страницы окон — если WebView2 открыл порт отладки (бывает не всегда).
    & "$root\node\node.exe" "$root\ui-check.mjs" 2>&1 | ForEach-Object { Log "$_"; if ("$_" -match '^FAIL') { $script:fails++ } }
    # Почему порта нет: с какими ключами запущены процессы WebView2 и слушает ли кто-то порт.
    Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -notmatch '--type=' } |
        ForEach-Object { Log ("WebView2 {0}: {1}" -f $_.ProcessId, $_.CommandLine) }
    Log ("порт 9341: " + ((Get-NetTCPConnection -LocalPort 9341 -State Listen -ErrorAction SilentlyContinue | ForEach-Object { "слушает процесс $($_.OwningProcess)" }) -join ', '))
    # Трей прячется, когда теряет фокус: щёлкнуть мимо — и он должен исчезнуть.
    [void][W]::SetCursorPos(200, 300); [W]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero); [W]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep 1
    $away = WindowsOf $win.Id
    Log ("окна kl!ck после щелчка мимо: " + $away)
    Check 'окно трея прячется, когда щёлкнули мимо' (-not ($away -split '; ' | Where-Object { $_ -match '^(3[2-4]\d)x(\d+) ' -and [int]$Matches[2] -gt 100 })) $away

    # 3a. Новое окно по макету: экраны и листы — как человек, щелчками по кнопкам (UI Automation
    # видит кнопки страницы по их подписям), снимок экрана после каждого шага.
    $main = [System.Windows.Automation.AutomationElement]::FromHandle($h)
    # Chromium строит дерево доступности по первому запросу: первый поиск часто пустой — ждём, пока появятся кнопки.
    function AllNamed { $main.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | Where-Object { $_.Current.Name } }
    for ($i = 0; $i -lt 20 -and @(AllNamed).Count -lt 5; $i++) { Start-Sleep -Milliseconds 500 }
    function FindName([string]$like) {
        for ($try = 0; $try -lt 6; $try++) {
            $hidden = $null
            foreach ($e in (AllNamed)) {
                if ($e.Current.Name -notlike $like) { continue }
                $r = $e.Current.BoundingRectangle
                if (-not $e.Current.IsOffscreen -and $r.Width -gt 0 -and $r.Height -gt 0) { return $e }
                if (-not $hidden) { $hidden = $e }
            }
            # Строка ниже видимой части окна: прокрутить к ней, как колёсиком.
            if ($hidden) {
                $sp = $null
                if ($hidden.TryGetCurrentPattern([System.Windows.Automation.ScrollItemPattern]::Pattern, [ref]$sp)) { $sp.ScrollIntoView(); Start-Sleep -Milliseconds 500; continue }
            }
            Start-Sleep -Milliseconds 500
        }
        $null
    }
    function ClickName([string]$like) {
        $e = FindName $like
        if (-not $e) { return $false }
        $r = $e.Current.BoundingRectangle
        [void][W]::SetForegroundWindow($h)
        [void][W]::SetCursorPos([int]($r.X + [Math]::Min($r.Width / 2, 40)), [int]($r.Y + $r.Height / 2)); Start-Sleep -Milliseconds 150
        [W]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero); Start-Sleep -Milliseconds 60; [W]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 900
        $true
    }
    function Esc { [void][W]::SetForegroundWindow($h); [W]::keybd_event(0x1B, 0, 0, [IntPtr]::Zero); [W]::keybd_event(0x1B, 0, 2, [IntPtr]::Zero); Start-Sleep -Milliseconds 700 }
    $steps = @(
        @('desk-speed', @('Скорость — подробнее')), @('', @('<esc>')),
        @('desk-add', @('Добавить подписку')), @('', @('<esc>')),
        @('desk-menu', @('Ещё')), @('desk-rename', @('Переименовать')), @('', @('<esc>')),
        @('desk-routes', @('Маршруты')),
        @('desk-routes-selected', @('Только выбранное*')),
        @('desk-rules', @('Мои сайты и приложения*')), @('', @('<esc>')),
        @('desk-routes-all', @('Всё, кроме исключений*')),
        @('desk-diag', @('Что-то не открывается*')),
        @('desk-settings', @('Настройки')),
        @('desk-method', @('Способ подключения*')), @('', @('<esc>')),
        @('desk-look', @('Оформление*')),
        @('desk-look-dark', @('Тёмная')),
        @('', @('Системная')),
        @('desk-killswitch', @('Настройки', 'Kill Switch?*')),
        @('desk-more', @('Настройки', 'Дополнительно*')),
        @('desk-about', @('Настройки', 'О приложении*')),
        @('', @('Главная'))
    )
    foreach ($s in $steps) {
        $fine = $true
        foreach ($c in $s[1]) {
            if ($c -eq '<esc>') { Esc; continue }
            if (-not (ClickName $c)) { $fine = $false; Check "окно: нашлась кнопка «$c»" $false }
        }
        if ($s[0]) { Shot $s[0]; if ($fine) { Log "снимок $($s[0])" } }
    }

    # 3b. Новые команды службы — напрямую по каналу управления, как их зовёт окно.
    function PipeCall([string]$json) {
        $c = New-Object System.IO.Pipes.NamedPipeClientStream('.', 'klick', [System.IO.Pipes.PipeDirection]::InOut)
        try {
            $c.Connect(5000)
            $enc = New-Object Text.UTF8Encoding $false
            $w = New-Object IO.StreamWriter($c, $enc); $w.AutoFlush = $true
            $r = New-Object IO.StreamReader($c, $enc)
            $w.WriteLine($json)
            for ($i = 0; $i -lt 50; $i++) {
                $line = $r.ReadLine()
                if ($null -eq $line) { break }
                $m = $line | ConvertFrom-Json
                if ($m.t -eq 'res') { return $m }
            }
        } catch { Log "канал: $($_.Exception.Message)" } finally { $c.Dispose() }
        $null
    }
    $conns = @((KlickJson settings).connections)
    $one = $conns | Where-Object { $_.kind -ne 'file' } | Select-Object -First 1
    $file = $conns | Where-Object { $_.kind -eq 'file' } | Select-Object -First 1
    if ($one) {
        $r = PipeCall ('{"id":1,"cmd":"rename_connection","args":{"id":"' + $one.id + '","name":"Переименовано"}}')
        $now = (@((KlickJson settings).connections) | Where-Object { $_.id -eq $one.id }).name
        Check 'служба: переименование' ($r.ok -and $now -eq 'Переименовано') "было «$($one.name)», стало «$now»"
        [void](PipeCall ('{"id":2,"cmd":"rename_connection","args":{"id":"' + $one.id + '","name":"' + $one.name + '"}}'))
        $r = PipeCall ('{"id":3,"cmd":"rename_connection","args":{"id":"' + $one.id + '","name":"   "}}')
        Check 'служба: пустое название не принимается' ((-not $r.ok) -and $r.error.code -eq 'input.empty_name') "$($r.error.code)"
        $r = PipeCall ('{"id":4,"cmd":"connection_link","args":{"id":"' + $one.id + '"}}')
        Check 'служба: «Скопировать ссылку» отдаёт ссылку' ($r.ok -and "$($r.data)" -match '^[a-z0-9]+://') ("$($r.data)" -replace '://.*', '://…')
    } else { Check 'есть подключение-ссылка для проверки' $false }
    if ($file) {
        $r = PipeCall ('{"id":5,"cmd":"connection_link","args":{"id":"' + $file.id + '"}}')
        Check 'служба: у файла ссылки нет' ((-not $r.ok) -and $r.error.code -eq 'conn.no_link') "$($r.error.code)"
    }
    $r = PipeCall '{"id":6,"cmd":"delay"}'
    Check 'служба: задержка без VPN — пусто' ($r.ok -and $null -eq $r.data.delay) ($r | ConvertTo-Json -Compress -Depth 4)

    # Левый щелчок — главное окно, даже если его закрыли крестиком (спрятали). Закрываем как человек,
    # WM_CLOSE: окно прячет само себя. ShowWindow(SW_HIDE) в обход окна сбил бы его учёт видимости,
    # и оно не показалось бы уже из-за проверки.
    [void][W]::SendMessage($h, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)   # WM_CLOSE
    Start-Sleep 1
    Check 'крестик прячет главное окно' (-not [W]::IsWindowVisible($h) -and -not $win.HasExited)
    Log ("трей: " + (ClickTrayIcon))
    Start-Sleep 2
    $left = WindowsOf $win.Id
    Check 'левый щелчок по значку открывает главное окно' ([W]::IsWindowVisible($h) -and $left -notmatch '^(3[2-4]\d)x\d{3,} |; (3[2-4]\d)x\d{3,} ') $left
    Shot 'desk-left-click'

    # 4. Развернуть — нельзя. Начинаем с обычного окна: свёрнутое «разворачивается» в обычное,
    # и сравнение размеров показало бы провал там, где его нет.
    [void][W]::ShowWindow($h, 9)   # SW_RESTORE
    Start-Sleep 1
    $before = RectOf $h
    [void][W]::SendMessage($h, 0x0112, [IntPtr]0xF030, [IntPtr]::Zero)   # WM_SYSCOMMAND, SC_MAXIMIZE
    Start-Sleep 1
    Check 'команда «Развернуть» не разворачивает' ((-not [W]::IsZoomed($h)) -and (RectOf $h) -eq $before) ("было {0}, стало {1}" -f $before, (RectOf $h))
    [void][W]::SetForegroundWindow($h)
    [W]::keybd_event(0x5B, 0, 0, [IntPtr]::Zero); [W]::keybd_event(0x26, 0, 0, [IntPtr]::Zero)
    [W]::keybd_event(0x26, 0, 2, [IntPtr]::Zero); [W]::keybd_event(0x5B, 0, 2, [IntPtr]::Zero)
    Start-Sleep 1
    Check 'Win+↑ не разворачивает' ((-not [W]::IsZoomed($h)) -and (RectOf $h) -eq $before) ("стало {0}" -f (RectOf $h))
    Shot 'desk-after-maximize'

    # 5. Чужой канал. В Песочнице всё запущено от администратора, а у таких процессов владелец
    # канала — «Администраторы», как у службы. Поэтому «чужого» запускаем с ограниченными правами
    # (runas /trustlevel) — как обычная программа у человека.
    Stop-Process -Id $win.Id -Force
    Stop-Service klick -Force
    Start-Sleep 2
    $pub = 'C:\Users\Public'
    Remove-Item "$pub\fake-pipe.txt" -ErrorAction SilentlyContinue
    Set-Content "$pub\fake-pipe.ps1" -Encoding UTF8 -Value @'
$s = New-Object System.IO.Pipes.NamedPipeServerStream('klick', [System.IO.Pipes.PipeDirection]::InOut, 10)
$task = $s.WaitForConnectionAsync()
$deadline = (Get-Date).AddSeconds(25)
while (-not $task.IsCompleted -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
$got = ''
if ($task.IsCompleted) {
    $buf = New-Object byte[] 4096
    $read = $s.ReadAsync($buf, 0, $buf.Length)
    if ($read.Wait(5000)) { $got = [Text.Encoding]::UTF8.GetString($buf, 0, $read.Result) }
    "connected; bytes: $($got.Length); $got" | Set-Content 'C:\Users\Public\fake-pipe.txt'
} else { 'nobody connected' | Set-Content 'C:\Users\Public\fake-pipe.txt' }
'@
    & runas.exe /trustlevel:0x20000 "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File $pub\fake-pipe.ps1" | Out-Null
    Start-Sleep 3
    $win2 = Start-Process "$inst\klick.exe" -ArgumentList '--hidden' -PassThru
    for ($i = 0; $i -lt 40 -and -not (Test-Path "$pub\fake-pipe.txt"); $i++) { Start-Sleep 1 }
    $fakeSaw = (Get-Content "$pub\fake-pipe.txt" -ErrorAction SilentlyContinue) -join ' '
    Check 'чужому каналу (программа без прав администратора) окно ничего не отправило' ($fakeSaw -and $fakeSaw -notmatch 'cmd') $fakeSaw
    Stop-Process -Id $win2.Id -Force -ErrorAction SilentlyContinue
    Get-Process powershell -ErrorAction SilentlyContinue | Where-Object { $_.Id -ne $PID } | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Service klick
    Start-Sleep 3

    # 6. Ссылка klick://add со страницы подписки: схема зарегистрирована, kl!ck запускается по ней,
    # а если уже работает — вторая копия передаёт ссылку первой и выходит.
    $cmd = (Get-ItemProperty 'HKCU:\Software\Classes\klick\shell\open\command' -ErrorAction SilentlyContinue).'(default)'
    Check 'схема klick:// ведёт в kl!ck' ("$cmd" -like "*$inst\klick.exe*%1*") "$cmd"
    Get-Process klick -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep 1
    Start-Process 'klick://add?url=https%3A%2F%2Fsub.example.com%2Fs%2Fsecret-1&name=%D0%A2%D0%B5%D1%81%D1%82'
    Start-Sleep 7
    $k = @(Get-Process klick -ErrorAction SilentlyContinue)
    $shown = $k.Count -eq 1 -and $k[0].MainWindowHandle -ne 0 -and [W]::IsWindowVisible($k[0].MainWindowHandle)
    Check 'ссылка запускает kl!ck и показывает окно' $shown "процессов: $($k.Count)"
    Shot 'desk-link-cold'
    [void][W]::SendMessage($k[0].MainWindowHandle, 0x0010, [IntPtr]::Zero, [IntPtr]::Zero)   # крестик: в трей
    Start-Sleep 1
    Start-Process 'klick://add/https://sub2.example.com/s/secret-2'
    Start-Sleep 5
    $k2 = @(Get-Process klick -ErrorAction SilentlyContinue)
    Check 'kl!ck уже работает: ссылка ушла в него, вторая копия вышла' ($k2.Count -eq 1 -and $k2[0].Id -eq $k[0].Id -and [W]::IsWindowVisible($k[0].MainWindowHandle)) "процессов: $($k2.Count)"
    Shot 'desk-link-warm'
    Start-Process "$inst\klick.exe"
    Start-Sleep 4
    Check 'второй запуск ярлыка не создаёт вторую копию' (@(Get-Process klick -ErrorAction SilentlyContinue).Count -eq 1) "процессов: $(@(Get-Process klick -ErrorAction SilentlyContinue).Count)"
    Start-Process 'klick://add?url=http%3A%2F%2Fsub.example.com%2Fs%2Fx'
    Start-Sleep -Milliseconds 1200   # сообщение «Ссылка не подходит» держится 2,8 с
    Check 'http-ссылка не принимается (окно на месте, копий нет)' (@(Get-Process klick -ErrorAction SilentlyContinue).Count -eq 1) ''
    Shot 'desk-link-bad'

}
catch {
    Log "ОШИБКА СЦЕНАРИЯ: $($_.Exception.Message)"
    $script:fails++
}
finally {
    Log "ИТОГ: провалов $($script:fails)"
    Copy-Item 'C:\ProgramData\klick\logs\service.log' "$out\service.log" -ErrorAction SilentlyContinue
    Set-Content "$out\script-done.txt" 'done'
    Start-Sleep 2
    shutdown.exe /s /t 0
}
