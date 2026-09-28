# Установщик, окно и трей глазами человека — в Песочнице Windows.
#   powershell -File sandbox\start.ps1 -Release -Script ui-check
# 1. «Прежняя kl!ck 0.2–0.4»: её зашифрованные подписки в %LOCALAPPDATA%\com.vbu00.klick.
# 2. klick-setup.exe --silent: ставит WebView2, если его нет, и переносит подписки.
# 3. Окно и трей: снимки всего экрана и самих страниц (протокол отладки WebView2).
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

Add-Type -AssemblyName System.Windows.Forms, System.Drawing, System.Security
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

    # 2. Установщик без окна
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
    & "$root\node\node.exe" "$root\ui-check.mjs" 2>&1 | ForEach-Object { Log "$_" }
    Shot 'desk-tray'

    # 4. Развернуть — нельзя
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

    # 5. Чужой канал: служба остановлена, имя \\.\pipe\klick заняла обычная программа
    Stop-Process -Id $win.Id -Force
    Stop-Service klick -Force
    Start-Sleep 2
    $fake = Start-Job {
        $s = New-Object System.IO.Pipes.NamedPipeServerStream('klick', [System.IO.Pipes.PipeDirection]::InOut, 10)
        $got = ''
        $deadline = (Get-Date).AddSeconds(20)
        $task = $s.WaitForConnectionAsync()
        while (-not $task.IsCompleted -and (Get-Date) -lt $deadline) { Start-Sleep -Milliseconds 200 }
        if ($task.IsCompleted) {
            $buf = New-Object byte[] 4096
            $read = $s.ReadAsync($buf, 0, $buf.Length)
            if ($read.Wait(5000)) { $got = [Text.Encoding]::UTF8.GetString($buf, 0, $read.Result) }
            "подключились; получено байт: $($got.Length); $got"
        } else { 'никто не подключился' }
    }
    Start-Sleep 1
    $win2 = Start-Process "$inst\klick.exe" -ArgumentList '--hidden' -PassThru
    Start-Sleep 12
    $fakeSaw = (Receive-Job $fake -Wait -AutoRemoveJob) -join ' '
    Check 'чужому каналу окно ничего не отправило' ($fakeSaw -notmatch 'cmd') $fakeSaw
    Stop-Process -Id $win2.Id -Force -ErrorAction SilentlyContinue
    Start-Service klick
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
