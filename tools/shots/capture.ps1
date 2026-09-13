# Screenshots of the running application, for the case study (SPEC.md §12.4, E4).
#
# WHY THIS DRIVES THE REAL APP AND NOT A BROWSER
#
# The obvious cheap route is `npm run dev` plus headless Chrome, which `tools/uiaudit`
# already does for the design gallery. It cannot work here: the search screen's every
# result comes through Tauri IPC, which does not exist in a browser, so a browser
# screenshot would show an empty state and call it evidence.
#
# So this launches the built binary, types into it as a person would, and captures the
# window. Slower, and it is the only version of this that proves anything.
#
# Usage:  powershell -ExecutionPolicy Bypass -File tools/shots/capture.ps1

param(
    [string]$OutDir = "docs/case-study/shots",
    [int]$SettleMs = 6000,
    [int]$QueryMs  = 2500
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# DwmGetWindowAttribute, NOT GetWindowRect.
#
# GetWindowRect includes the invisible resize border and drop shadow a composited window
# carries, so the first run captured 78 px of desktop down the left and 45 px along the
# top — with the browser behind it plainly visible — and cut the same amount off the
# right, taking the "why this matched" badges with it. A screenshot with someone's
# YouTube tab in the margin is not evidence of anything.
#
# DWMWA_EXTENDED_FRAME_BOUNDS (9) is the rectangle actually painted.
$signature = @'
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr hWnd, int attr, out RECT rect, int size);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
'@
Add-Type -MemberDefinition $signature -Name Win -Namespace Native

function Capture-Window {
    param([IntPtr]$Handle, [string]$Path)

    $rect = New-Object Native.Win+RECT
    $DWMWA_EXTENDED_FRAME_BOUNDS = 9
    [void][Native.Win]::DwmGetWindowAttribute(
        $Handle, $DWMWA_EXTENDED_FRAME_BOUNDS, [ref]$rect, [System.Runtime.InteropServices.Marshal]::SizeOf($rect))
    $width  = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -le 0 -or $height -le 0) { throw "window has no size; is it minimised?" }

    $bitmap   = New-Object System.Drawing.Bitmap $width, $height
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
    $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose(); $bitmap.Dispose()
    Write-Host ("  {0}  {1}x{2}" -f (Split-Path $Path -Leaf), $width, $height)
}

# The queries E4 names, plus the two that show the parts of the engine E4 does not ask
# about — a filter query, and an exact title, which is the case E2 guarantees.
$shots = @(
    @{ name = "01-grief";       query = "films about grief that aren't depressing" },
    @{ name = "02-wong-kar-wai"; query = "like Wong Kar-wai but Korean" },
    @{ name = "03-filters";     query = "films directed by Alfred Hitchcock from the 1950s" },
    @{ name = "04-exact-title"; query = "Seven Samurai" }
)

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
Write-Host "shots: launching the release build"

$process = Start-Process -FilePath "target\release\sin-e-phile.exe" -PassThru
try {
    Start-Sleep -Milliseconds $SettleMs
    $process.Refresh()
    if ($process.MainWindowHandle -eq [IntPtr]::Zero) { throw "no window appeared" }
    $handle = $process.MainWindowHandle

    foreach ($shot in $shots) {
        [void][Native.Win]::SetForegroundWindow($handle)
        Start-Sleep -Milliseconds 400

        # "/" enters search from anywhere; Escape leaves it. Sent as real keystrokes
        # rather than injected state, so what is captured is what a person would get.
        [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
        Start-Sleep -Milliseconds 200
        [System.Windows.Forms.SendKeys]::SendWait("/")
        Start-Sleep -Milliseconds 400

        # SendKeys reads + ^ % ~ ( ) { } as syntax, so anything a film title might
        # contain is escaped.
        $escaped = $shot.query -replace '([+^%~(){}\[\]])', '{$1}'
        [System.Windows.Forms.SendKeys]::SendWait($escaped)

        # Long enough for the 120 ms debounce, the query embedding and the search.
        Start-Sleep -Milliseconds $QueryMs
        Capture-Window -Handle $handle -Path (Join-Path $OutDir "$($shot.name).png")
    }
}
finally {
    if (-not $process.HasExited) { $process.Kill() }
}

Write-Host "shots: done"
