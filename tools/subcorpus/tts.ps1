# Speak each line of a script into its own WAV, 16 kHz mono 16-bit, with Windows' own
# speech synthesiser (System.Speech, no download). Used once to build the subtitle
# corpus; the traces it leads to are committed, so CI never needs a voice.
#
#   powershell -File tools/subcorpus/tts.ps1 -Script lines.tsv -Out dir
#
# lines.tsv: one line per utterance, "<index>\t<voice>\t<rate>\t<text>".
param([string]$Script, [string]$Out)
Add-Type -AssemblyName System.Speech
$format = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
foreach ($line in Get-Content -Encoding UTF8 $Script) {
    $f = $line.Split("`t")
    $synth.SelectVoice($f[1])
    $synth.Rate = [int]$f[2]
    $synth.SetOutputToWaveFile((Join-Path $Out ("{0}.wav" -f $f[0])), $format)
    $synth.Speak($f[3])
}
$synth.SetOutputToNull()
