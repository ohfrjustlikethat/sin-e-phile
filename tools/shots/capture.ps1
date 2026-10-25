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
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, IntPtr pid);
[DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
[DllImport("user32.dll")] public static extern bool AttachThreadInput(uint from, uint to, bool attach);
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

    # MainWindowHandle is whichever top-level window .NET saw first, and the webview
    # creates small helper windows. One run captured all four shots at 219x30. Wait for
    # a handle whose painted frame is window-sized, and fail loudly if none appears.
    $handle = [IntPtr]::Zero
    for ($i = 0; $i -lt 40; $i++) {
        $process.Refresh()
        if ($process.HasExited) { throw "the app exited before its window appeared" }
        $candidate = $process.MainWindowHandle
        if ($candidate -ne [IntPtr]::Zero) {
            # A window launched from a background process can come up minimised (seen
            # 2026-10-25: 158x26, parked off-screen). SW_RESTORE (9) brings it back.
            [void][Native.Win]::ShowWindow($candidate, 9)
            Start-Sleep -Milliseconds 300
            $rect = New-Object Native.Win+RECT
            [void][Native.Win]::DwmGetWindowAttribute(
                $candidate, 9, [ref]$rect, [System.Runtime.InteropServices.Marshal]::SizeOf($rect))
            if (($rect.Right - $rect.Left) -ge 800) { $handle = $candidate; break }
        }
        Start-Sleep -Milliseconds 500
    }
    if ($handle -eq [IntPtr]::Zero) { throw "no window-sized window appeared" }

    foreach ($shot in $shots) {
        [void][Native.Win]::ShowWindow($handle, 9)
        # Windows refuses SetForegroundWindow from a process that is not in front. The
        # documented way round it: attach to the input queue of the window that IS in
        # front for the duration of the call, then detach.
        $front = [Native.Win]::GetWindowThreadProcessId([Native.Win]::GetForegroundWindow(), [IntPtr]::Zero)
        $mine = [Native.Win]::GetCurrentThreadId()
        [void][Native.Win]::AttachThreadInput($mine, $front, $true)
        [void][Native.Win]::SetForegroundWindow($handle)
        [void][Native.Win]::AttachThreadInput($mine, $front, $false)
        Start-Sleep -Milliseconds 600

        # SendKeys types into WHATEVER window is in front. Windows may refuse to bring a
        # background process's window forward, and then the query would be typed into
        # someone's terminal or editor. Never send a key unless the app is in front.
        if ([Native.Win]::GetForegroundWindow() -ne $handle) {
            throw "the app is not the foreground window, so no keys were sent ($($shot.name))"
        }

        # "/" enters search from anywhere; Escape leaves it. Sent as real keystrokes
        # rather than injected state, so what is captured is what a person would get.
        [System.Windows.Forms.SendKeys]::SendWait("{ESC}")
        Start-Sleep -Milliseconds 600
        [System.Windows.Forms.SendKeys]::SendWait("/")
        # Long enough for the search screen to mount and take focus. At 400 ms the query
        # sometimes arrived first, and its first space pressed the focused title-bar
        # Minimise button.
        Start-Sleep -Milliseconds 1000

        # SendKeys reads + ^ % ~ ( ) { } as syntax, so anything a film title might
        # contain is escaped.
        $escaped = $shot.query -replace '([+^%~(){}\[\]])', '{$1}'
        [System.Windows.Forms.SendKeys]::SendWait($escaped)

        # Park the pointer in the screen's corner. Resting over the maximise button, it
        # opened Windows' Snap Layouts flyout across the results in one capture.
        [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point 0, 0

        # Long enough for the 120 ms debounce, the query embedding and the search.
        Start-Sleep -Milliseconds $QueryMs
        Capture-Window -Handle $handle -Path (Join-Path $OutDir "$($shot.name).png")
    }
}
finally {
    if (-not $process.HasExited) { $process.Kill() }
}

Write-Host "shots: done"
