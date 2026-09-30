# Окно kl!ck глазами проверок: что на экране (имена элементов страницы через UI Automation — WebView2
# отдаёт их, как браузер), нажать кнопку, снимок экрана. Подключается: . "$PSScriptRoot\ui.ps1"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

function KlickProcs { @(Get-Process klick -ErrorAction SilentlyContinue) }

function UiWindows([int]$procId) {
    $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ProcessIdProperty, $procId)
    [System.Windows.Automation.AutomationElement]::RootElement.FindAll([System.Windows.Automation.TreeScope]::Children, $cond)
}

# Все тексты в окнах процесса через « | ».
function UiText([int]$procId) {
    $names = foreach ($w in (UiWindows $procId)) {
        try { foreach ($e in $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)) { $e.Current.Name } } catch {}
    }
    ($names | Where-Object { $_ }) -join ' | '
}

# Подождать, пока в окне появится один из текстов; вернуть всё, что видно.
# На холодном старте WebView2 (Windows Server 2022) дерево страницы может так и не появиться в
# UI Automation, пока окно не получит фокус. Поэтому раз в 10 с без нужного текста окно получает
# фокус, как от щелчка пользователя, и это пишется в отчёт.
function WaitUi([int]$procId, [string[]]$any, [int]$seconds = 20) {
    $end = (Get-Date).AddSeconds($seconds)
    $nudge = (Get-Date).AddSeconds(10)
    do {
        $t = UiText $procId
        foreach ($a in $any) { if ($t.Contains($a)) { return $t } }
        if ((Get-Date) -gt $nudge) {
            foreach ($w in (UiWindows $procId)) { try { $w.SetFocus() } catch {} }
            if (Get-Command Log -ErrorAction SilentlyContinue) { Log '    текста страницы в окне нет: фокус на окно' }
            $nudge = (Get-Date).AddSeconds(10)
        }
        Start-Sleep -Milliseconds 300
    } while ((Get-Date) -lt $end)
    $t
}

# Нажать кнопку по имени (тексту или aria-label).
function UiPress([int]$procId, [string]$name) {
    $cond = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $name)),
        (New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::ControlTypeProperty, [System.Windows.Automation.ControlType]::Button)))
    foreach ($w in (UiWindows $procId)) {
        $b = $w.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
        if ($b) { $b.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); return $true }
    }
    $false
}

function Screenshot([string]$file) {
    Add-Type -AssemblyName System.Windows.Forms, System.Drawing
    $b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bmp = New-Object System.Drawing.Bitmap $b.Width, $b.Height
    [System.Drawing.Graphics]::FromImage($bmp).CopyFromScreen($b.Location, [System.Drawing.Point]::Empty, $b.Size)
    $bmp.Save($file)
}

# Дождаться первого окна kl!ck после запуска по ссылке; $null — не появилось.
function WaitKlick([int]$seconds = 30) {
    $end = (Get-Date).AddSeconds($seconds)
    while (-not (KlickProcs) -and (Get-Date) -lt $end) { Start-Sleep -Milliseconds 300 }
    KlickProcs | Select-Object -First 1
}
