# Проверка установщика kl!ck внутри Песочницы Windows. Запускается сама при входе (LogonCommand).
# Прежняя kl!ck (0.2) → установка klick-setup.exe → все проверки службы на установленной копии (run.ps1)
# → переустановка с занятой папкой (откат) и без → удаление копией из папки программы (данные остаются)
# → установка в свою папку без ярлыков → удаление с данными → окно установщика.

$ErrorActionPreference = 'Continue'
$root = 'C:\klick'
$out = Join-Path $root 'results'
$inst = 'C:\Program Files\klick'
$oldDir = 'C:\Program Files\kl!ck'
$custom = 'C:\Apps\klick'
New-Item -ItemType Directory -Force $out | Out-Null
$report = Join-Path $out 'setup-report.txt'
Set-Content -Path $report -Value '' -Encoding UTF8
$script:fails = 0

# Отчёт лежит в общей с компьютером папке: если его кто-то читает, запись ненадолго занята — повторить.
function Log([string]$m) {
    $line = "[{0:HH:mm:ss}] {1}" -f (Get-Date), $m
    for ($i = 0; $i -lt 20; $i++) {
        try { Add-Content -Path $report -Value $line -Encoding UTF8 -ErrorAction Stop; return } catch { Start-Sleep -Milliseconds 250 }
    }
}
function Check([string]$name, [bool]$ok, [string]$detail = '') {
    if (-not $ok) { $script:fails++ }
    $mark = if ($ok) { 'OK  ' } else { 'FAIL' }
    $tail = if ($detail) { " - $detail" } else { '' }
    Log "$mark $name$tail"
}
# Установщик без окна; вывод — в журнал. Возвращает код выхода.
function Setup([string[]]$a, [string]$exe = "$root\klick-setup.exe") {
    $p = Start-Process -FilePath $exe -ArgumentList $a -Wait -PassThru -WindowStyle Hidden -RedirectStandardOutput "$out\setup-out.txt" -RedirectStandardError "$out\setup-err.txt"
    $text = ((Get-Content "$out\setup-out.txt" -Encoding UTF8 -ErrorAction SilentlyContinue) + (Get-Content "$out\setup-err.txt" -Encoding UTF8 -ErrorAction SilentlyContinue)) -join "`n"
    Log ("klick-setup {0} -> код {1}`n{2}" -f ($a -join ' '), $p.ExitCode, $text.Trim())
    return $p.ExitCode
}
function RegVal([string]$path, [string]$name) { (Get-ItemProperty -LiteralPath $path -Name $name -ErrorAction SilentlyContinue).$name }
function RegHas([string]$path) { Test-Path -LiteralPath $path }
function LnkTarget([string]$lnk) { if (Test-Path -LiteralPath $lnk) { (New-Object -ComObject WScript.Shell).CreateShortcut($lnk).TargetPath } }
function LnkAumid([string]$lnk) {
    if (-not (Test-Path -LiteralPath $lnk)) { return $null }
    $f = (New-Object -ComObject Shell.Application).NameSpace((Split-Path $lnk))
    $f.ParseName((Split-Path $lnk -Leaf)).ExtendedProperty('System.AppUserModel.ID')
}
function WintunPkgs { @(Get-ChildItem "$env:WINDIR\INF\oem*.inf" -ErrorAction SilentlyContinue | Where-Object { (Get-Content $_.FullName -Raw -ErrorAction SilentlyContinue) -match 'wintun\.sys' }) }
function Http([string]$exe, [string]$url = 'https://www.gstatic.com/generate_204') {
    $code = & $exe -s -o NUL -w '%{http_code}' --max-time 12 $url 2>$null
    if ($code) { "$code".Trim() } else { '000' }
}
function Svc { Get-Service klick -ErrorAction SilentlyContinue }
# Копию установщика из папки программы и саму папку убирают через секунду после выхода — подождать.
function Gone([string]$p, [int]$seconds = 15) { $end = (Get-Date).AddSeconds($seconds); while ((Test-Path -LiteralPath $p) -and (Get-Date) -lt $end) { Start-Sleep -Milliseconds 500 }; -not (Test-Path -LiteralPath $p) }

$uninst = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\klick'
$oldUninst = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\kl!ck'
$is = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings'
$run = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$approved = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run'
$notif = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings'
$tray = 'HKCU:\Control Panel\NotifyIconSettings'
$startLnk = 'C:\ProgramData\Microsoft\Windows\Start Menu\Programs\kl!ck.lnk'
$publicLnk = 'C:\Users\Public\Desktop\kl!ck.lnk'
$userLnk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'kl!ck.lnk'

try {
    Log ("Песочница: Windows {0}" -f [Environment]::OSVersion.Version)

    # 0. Прежняя kl!ck 0.2.1: файлы, её деинсталлятор, «ядро» работает, задача, правило брандмауэра,
    #    системный прокси поверх «корпоративного», настройки в профиле, ярлыки, значок трея, уведомления.
    New-Item -ItemType Directory -Force "$oldDir\bin" | Out-Null
    Copy-Item "$root\old\*" $oldDir -Recurse -Force
    Copy-Item "$env:WINDIR\System32\PING.EXE" "$oldDir\bin\mihomo.exe" -Force
    $fakeCore = Start-Process "$oldDir\bin\mihomo.exe" -ArgumentList '-t', '127.0.0.1' -WindowStyle Hidden -PassThru
    New-Item -Path $oldUninst -Force | Out-Null
    Set-ItemProperty -LiteralPath $oldUninst -Name DisplayName -Value 'kl!ck'
    Set-ItemProperty -LiteralPath $oldUninst -Name DisplayVersion -Value '0.2.1'
    Set-ItemProperty -LiteralPath $oldUninst -Name Publisher -Value 'vbu00'
    Set-ItemProperty -LiteralPath $oldUninst -Name InstallLocation -Value "`"$oldDir`""
    Set-ItemProperty -LiteralPath $oldUninst -Name UninstallString -Value "`"$oldDir\uninstall.exe`""
    New-Item -Path 'HKLM:\SOFTWARE\vbu00\kl!ck' -Force | Out-Null
    Set-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\vbu00\kl!ck' -Name '(default)' -Value $oldDir
    New-Item -Path 'HKLM:\SOFTWARE\vbu00\Klutz' -Force | Out-Null
    Set-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\vbu00\Klutz' -Name probe -Value 'keep'
    $ws = New-Object -ComObject WScript.Shell
    foreach ($l in @($startLnk, $userLnk)) { $s = $ws.CreateShortcut($l); $s.TargetPath = "$oldDir\klick.exe"; $s.Save() }
    # Через cmdlet: кавычки внутри /TR у schtasks PowerShell 5.1 передаёт неверно.
    $action = New-ScheduledTaskAction -Execute "$oldDir\klick.exe" -Argument '--autostart'
    Register-ScheduledTask -TaskName 'klick-Autostart' -Action $action -Trigger (New-ScheduledTaskTrigger -AtLogOn) -RunLevel Highest -Force | Out-Null
    New-NetFirewallRule -DisplayName 'klick-ks-probe' -Group 'klick-killswitch' -Direction Outbound -Action Block -Program 'C:\oldks\curl.exe' | Out-Null
    $od = "$env:LOCALAPPDATA\com.vbu00.klick"
    New-Item -ItemType Directory -Force "$od\EBWebView" | Out-Null
    Set-Content "$od\settings.json" '{"mode":"sysproxy","proxyPort":7890}' -Encoding ASCII
    Set-Content "$od\sysproxy-backup.json" '{"enable":1,"server":"10.0.0.1:3128","overrides":"<local>"}' -Encoding ASCII
    Set-Content "$od\profiles.dat" 'secret' -Encoding ASCII
    Set-ItemProperty -LiteralPath $is -Name ProxyServer -Value '127.0.0.1:7890'
    Set-ItemProperty -LiteralPath $is -Name ProxyOverride -Value 'localhost'
    Set-ItemProperty -LiteralPath $is -Name ProxyEnable -Value 1 -Type DWord
    New-Item -Path "$notif\com.vbu00.klick" -Force | Out-Null
    New-Item -Path "$notif\com.vbu00.klutz" -Force | Out-Null
    New-Item -Path "$tray\1111" -Force | Out-Null
    Set-ItemProperty -LiteralPath "$tray\1111" -Name ExecutablePath -Value '{6D809377-6AF0-444B-8957-A3773F02200E}\kl!ck\klick.exe'
    New-Item -Path "$tray\2222" -Force | Out-Null
    Set-ItemProperty -LiteralPath "$tray\2222" -Name ExecutablePath -Value 'C:\Windows\explorer.exe'
    $oldTask = [bool](Get-ScheduledTask -TaskName 'klick-Autostart' -ErrorAction SilentlyContinue)
    $oldRule = [bool](Get-NetFirewallRule -Group 'klick-killswitch' -ErrorAction SilentlyContinue)
    $oldFiles = (Test-Path -LiteralPath "$oldDir\klick.exe") -and (Test-Path -LiteralPath "$oldDir\uninstall.exe")
    Check 'прежняя kl!ck на месте (файлы, деинсталлятор, задача, правило, «ядро»)' ($oldFiles -and $oldTask -and $oldRule -and -not $fakeCore.HasExited) ("файлы {0}, задача {1}, правило {2}" -f $oldFiles, $oldTask, $oldRule)

    # 1. Установка поверх прежней kl!ck
    $code = Setup @('--silent')
    Check 'установка: код 0' ($code -eq 0) "код $code"
    Check 'файлы программы на месте' ((Test-Path "$inst\klick.exe") -and (Test-Path "$inst\klick-service.exe") -and (Test-Path "$inst\klick-setup.exe") -and (Test-Path "$inst\resources\core\mihomo.exe") -and (Test-Path "$inst\resources\licenses\GPL-3.0.txt"))
    Check 'служба работает' ((Svc).Status -eq 'Running')
    Check 'запись в «Установленных приложениях»' ((RegVal $uninst 'DisplayVersion') -and (RegVal $uninst 'InstallLocation') -eq $inst -and (RegVal $uninst 'UninstallString') -like '*klick-setup.exe*--uninstall') ("версия {0}, {1}" -f (RegVal $uninst 'DisplayVersion'), (RegVal $uninst 'UninstallString'))
    Check 'ярлык в «Пуске» ведёт в новую папку' ((LnkTarget $startLnk) -eq "$inst\klick.exe") (LnkTarget $startLnk)
    Check 'у ярлыка код приложения для уведомлений' ((LnkAumid $startLnk) -eq 'app.klick.desktop') ("AUMID " + (LnkAumid $startLnk))
    Check 'ярлык на рабочем столе' ((LnkTarget $publicLnk) -eq "$inst\klick.exe")
    Check 'автозапуск свёрнутым' ((RegVal $run 'kl!ck') -eq "`"$inst\klick.exe`" --hidden" -and (RegHas $approved) -and $null -ne (RegVal $approved 'kl!ck')) (RegVal $run 'kl!ck')
    Check 'прежняя kl!ck: папки нет' (-not (Test-Path -LiteralPath $oldDir))
    Check 'прежняя kl!ck: «ядро» остановлено' ($fakeCore.HasExited)
    Check 'прежняя kl!ck: записи в реестре нет' (-not (RegHas $oldUninst) -and -not (RegHas 'HKLM:\SOFTWARE\vbu00\kl!ck'))
    Check 'соседняя программа издателя (Klutz) не тронута' ((RegVal 'HKLM:\SOFTWARE\vbu00\Klutz' 'probe') -eq 'keep')
    Check 'прежняя kl!ck: задачи автозапуска нет' (-not (Get-ScheduledTask -TaskName 'klick-Autostart' -ErrorAction SilentlyContinue))
    Check 'прежняя kl!ck: правил брандмауэра нет' (-not (Get-NetFirewallRule -Group 'klick-killswitch' -ErrorAction SilentlyContinue))
    Check 'прежняя kl!ck: настроек в профиле нет' (-not (Test-Path $od))
    $proxy = Get-ItemProperty -LiteralPath $is
    Check 'прежняя kl!ck: вернулся прокси, что был до неё' ($proxy.ProxyServer -eq '10.0.0.1:3128' -and $proxy.ProxyEnable -eq 1) ("{0}, включён {1}" -f $proxy.ProxyServer, $proxy.ProxyEnable)
    Check 'прежняя kl!ck: ярлыков нет' (-not (Test-Path -LiteralPath $userLnk))
    Check 'прежняя kl!ck: значок трея забыт, чужой на месте' (-not (RegHas "$tray\1111") -and (RegHas "$tray\2222"))
    Check 'прежняя kl!ck: уведомления забыты, Klutz на месте' (-not (RegHas "$notif\com.vbu00.klick") -and (RegHas "$notif\com.vbu00.klutz"))
    Set-ItemProperty -LiteralPath $is -Name ProxyEnable -Value 0 -Type DWord

    # 2. Все проверки службы — на установленной копии
    Log '--- run.ps1 -Preinstalled -KeepInstalled ---'
    & "$root\run.ps1" -Preinstalled -KeepInstalled
    $sum = (Get-Content "$out\report.txt" -Encoding UTF8 | Select-String 'ИТОГ: провалов (\d+)' | Select-Object -Last 1)
    $runFails = if ($sum) { [int]$sum.Matches[0].Groups[1].Value } else { -1 }
    Check 'проверки службы на установленной копии (run.ps1)' ($runFails -eq 0) ("провалов: $runFails")
    Check 'Kill Switch держит программу без сети перед удалением' ((Http 'C:\kstest\curl.exe') -eq '000')
    $drivers = (WintunPkgs).Count
    Log "пакетов Wintun в хранилище драйверов: $drivers"

    # 3. Переустановка: папку держит чужой процесс — ошибка и откат; потом без помех
    $lock = [IO.File]::Open("$inst\resources\catalog.json", 'Open', 'Read', 'None')
    $code = Setup @('--silent')
    $lock.Close()
    Check 'занятая папка: установщик сообщил об ошибке' ($code -eq 1) "код $code"
    Check 'после ошибки служба снова работает' ((Svc).Status -eq 'Running')
    Check 'после ошибки файлы на месте, копии нет' ((Test-Path "$inst\klick-service.exe") -and -not (Test-Path "$inst.old"))
    $code = Setup @('--silent')
    Check 'переустановка: код 0' ($code -eq 0) "код $code"
    Check 'переустановка: служба работает, копии прежних файлов нет' ((Svc).Status -eq 'Running' -and -not (Test-Path "$inst.old"))

    # 4. Удаление копией из папки программы, данные остаются. Перед этим — «забытые» следы окна.
    New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\klick" | Out-Null
    Set-Content "$env:LOCALAPPDATA\klick\proxy-backup.json" '{"enable":1,"server":"10.0.0.2:8080","bypass":"<local>"}' -Encoding ASCII
    Set-ItemProperty -LiteralPath $is -Name ProxyServer -Value '127.0.0.1:7890'
    Set-ItemProperty -LiteralPath $is -Name ProxyEnable -Value 1 -Type DWord
    New-Item -Path "$tray\3333" -Force | Out-Null
    Set-ItemProperty -LiteralPath "$tray\3333" -Name ExecutablePath -Value '{6D809377-6AF0-444B-8957-A3773F02200E}\klick\klick.exe'
    New-Item -Path "$notif\app.klick.desktop" -Force | Out-Null
    $code = Setup @('--uninstall', '--silent') "$inst\klick-setup.exe"
    Check 'удаление: код 0' ($code -eq 0) "код $code"
    Check 'служба удалена' (-not (Svc))
    Check 'папки программы нет' (Gone $inst) ((@(Get-ChildItem $inst -Recurse -ErrorAction SilentlyContinue) | ForEach-Object { $_.Name }) -join ', ')
    Check 'записи в «Установленных приложениях» нет' (-not (RegHas $uninst))
    Check 'ярлыков нет' (-not (Test-Path -LiteralPath $startLnk) -and -not (Test-Path -LiteralPath $publicLnk))
    Check 'автозапуска нет' ($null -eq (RegVal $run 'kl!ck') -and $null -eq (RegVal $approved 'kl!ck'))
    Check 'значок трея забыт, чужой на месте' (-not (RegHas "$tray\3333") -and (RegHas "$tray\2222"))
    Check 'уведомления забыты' (-not (RegHas "$notif\app.klick.desktop"))
    $proxy = Get-ItemProperty -LiteralPath $is
    Check 'системный прокси вернулся прежний' ($proxy.ProxyServer -eq '10.0.0.2:8080' -and $proxy.ProxyEnable -eq 1 -and -not (Test-Path "$env:LOCALAPPDATA\klick\proxy-backup.json")) ("{0}, включён {1}" -f $proxy.ProxyServer, $proxy.ProxyEnable)
    Set-ItemProperty -LiteralPath $is -Name ProxyEnable -Value 0 -Type DWord
    $ks = Http 'C:\kstest\curl.exe'
    Check 'Kill Switch снят: программа снова в сети' ($ks -eq '204') "код $ks"
    $left = (WintunPkgs).Count
    Check 'драйвер Wintun убран из системы' ($left -eq 0) "было $drivers, осталось $left"
    Check 'данные остались' (Test-Path 'C:\ProgramData\klick\settings.json')
    Start-Sleep 7
    $tmp = @(Get-ChildItem $env:TEMP -Filter 'klick-setup-*.exe' -ErrorAction SilentlyContinue)
    Check 'временная копия установщика удалила себя' ($tmp.Count -eq 0) ($tmp.Name -join ', ')

    # 5. Установка в свою папку без ярлыка и автозапуска; профили подхватываются
    $code = Setup @('--silent', '--path', $custom, '--no-desktop', '--no-autostart')
    Check 'установка в свою папку: код 0' ($code -eq 0) "код $code"
    Check 'служба работает из своей папки' ((Svc).Status -eq 'Running' -and (Get-CimInstance Win32_Service -Filter "Name='klick'").PathName -like "*$custom*")
    Check 'без ярлыка на рабочем столе, в «Пуске» есть' (-not (Test-Path -LiteralPath $publicLnk) -and (LnkTarget $startLnk) -eq "$custom\klick.exe")
    Check 'без автозапуска' ($null -eq (RegVal $run 'kl!ck'))
    $acl = (Get-Acl $custom).Access
    $users = @($acl | Where-Object { $_.IdentityReference.Value -match 'Users$|Пользователи$' })
    $canWrite = @($users | Where-Object { $_.FileSystemRights -match 'Write|Modify|FullControl' })
    Check 'своя папка закрыта от изменений, как Program Files' ($users.Count -gt 0 -and $canWrite.Count -eq 0 -and -not ($acl | Where-Object { $_.IsInherited })) (($users | ForEach-Object { "$($_.IdentityReference): $($_.FileSystemRights)" }) -join '; ')
    Copy-Item "$root\bin\klick-cli.exe" $custom -Force
    $servers = @(& "$custom\klick-cli.exe" --prod servers 2>$null | Out-String | ConvertFrom-Json | ForEach-Object { $_ })
    Check 'профили и серверы подхватились после переустановки' ($servers.Count -ge 1) ("серверов: " + $servers.Count)

    # 6. Удаление вместе с данными — установщиком снаружи
    $code = Setup @('--silent', '--uninstall', '--wipe')
    Check 'удаление с данными: код 0' ($code -eq 0) "код $code"
    Check 'своей папки нет' (Gone $custom)
    Check 'данных службы нет' (-not (Test-Path 'C:\ProgramData\klick'))
    Check 'данных окна нет' (-not (Test-Path "$env:LOCALAPPDATA\klick") -and -not (Test-Path "$env:LOCALAPPDATA\app.klick.desktop"))
    Check 'служба удалена' (-not (Svc))

    # 7. Окно установщика
    $ui = Start-Process "$root\klick-setup.exe" -PassThru
    Start-Sleep 8
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save("$out\setup-window.png")
    Check 'окно установщика открылось' (-not $ui.HasExited)
    Check 'окно установщика не пишет в профиль' (-not (Test-Path "$env:LOCALAPPDATA\app.klick.setup"))
    Stop-Process -Id $ui.Id -Force -ErrorAction SilentlyContinue
}
catch {
    $script:fails++
    Log "ОШИБКА СЦЕНАРИЯ: $($_.Exception.Message)"
}
finally {
    Log ("ИТОГ УСТАНОВЩИКА: провалов {0}" -f $script:fails)
    Set-Content "$out\setup-done.txt" 'done'
    Start-Sleep 2
    shutdown.exe /s /t 0
}
