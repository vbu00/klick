# Озвучка: lines.json ([{id, say}]) -> <id>.wav и durations.json ({id: секунды}).
param([string]$Dir, [string]$Voice = 'Microsoft Irina Desktop', [int]$Rate = 1)
Add-Type -AssemblyName System.Speech
$lines = Get-Content -Raw -Encoding UTF8 (Join-Path $Dir 'lines.json') | ConvertFrom-Json
$out = @{}
foreach ($l in $lines) {
    $s = New-Object System.Speech.Synthesis.SpeechSynthesizer
    $s.SelectVoice($Voice)
    $s.Rate = $Rate
    $wav = Join-Path $Dir ($l.id + '.wav')
    $s.SetOutputToWaveFile($wav)
    $s.Speak($l.say)
    $s.Dispose()
    $bytes = (Get-Item $wav).Length
    # PCM, заголовок 44 байта; частота и разрядность — из заголовка.
    $fs = [IO.File]::OpenRead($wav); $br = New-Object IO.BinaryReader($fs)
    $fs.Position = 28; $byteRate = $br.ReadInt32(); $br.Close()
    $out[$l.id] = [math]::Round(($bytes - 44) / $byteRate, 2)
}
$out | ConvertTo-Json | Set-Content -Encoding UTF8 (Join-Path $Dir 'durations.json')
"ok: $($lines.Count)"
