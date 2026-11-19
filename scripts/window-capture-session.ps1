<#
.SYNOPSIS
  One window capture of ShareXrs's framed test window, over the test
  pattern, in ShareX or ShareXrs: covers the monitors with
  `sharexrs --test-pattern --framed X,Y,W,H`, makes the framed window the
  foreground one (a click in its client area) and sends the running app a
  command line job: -ActiveWindow, or -RectangleRegion followed by a click
  on the window (the region overlay snaps to it and captures it whole).
  -Job Menu chooses Capture > Window > the framed window from the app's
  menu instead (UI Automation in ShareX, clicks in ShareXrs's window, as
  capture-session.ps1 does): a forwarded -ActiveWindow brings ShareX's
  own window to the front first.
  New files are pattern-checked by their opaque pixels (deleted if they
  aren't mostly the pattern) and copied to -Out.

  While it runs it watches, through the API, whether the taskbar
  (IsWindowVisible of Shell_TrayWnd) and the desktop icons (the desktop's
  SysListView32 "FolderView") are visible, and prints every change. They
  end as they started: if either isn't, the finally block shows or hides
  it again and says so.

.EXAMPLE
  scripts/window-capture-session.ps1 -Process ShareX -Job ActiveWindow -Framed 300,200,400,300 -Out G:/x/sharex
#>
param(
    [Parameter(Mandatory)] [ValidateSet('ShareX', 'sharexrs')] [string] $Process,
    # CustomWindow captures the window titled CaptureCustomWindow (set it to
    # 'ShareXrs framed test window'): found by title, restored, activated.
    [ValidateSet('ActiveWindow', 'CustomWindow', 'RectangleRegion', 'Menu')] [string] $Job = 'CustomWindow',
    [int[]] $Framed = @(300, 200, 400, 300),
    [string] $Out,
    # Pattern share the capture's opaque pixels need (the title bar isn't).
    [double] $Min = 0.6,
    [int] $WaitMs = 20000,
    [string] $Sharexrs = "$PSScriptRoot/../target/debug/sharexrs.exe",
    [string] $ShareXExe = 'G:/scripts/sharexrs-ref/ShareX-build/ShareX.exe'
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class WcsWin {
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr a, string c, string t);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out RECT r, int s);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    public static POINT Client(string title) {
        IntPtr h = FindWindowExW(IntPtr.Zero, IntPtr.Zero, null, title);
        POINT p = new POINT(); ClientToScreen(h, ref p); return p;
    }
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static IntPtr Taskbar() { return FindWindowExW(IntPtr.Zero, IntPtr.Zero, "Shell_TrayWnd", null); }
    public static IntPtr Icons() {
        IntPtr progman = FindWindowExW(IntPtr.Zero, IntPtr.Zero, "Progman", null);
        IntPtr view = FindWindowExW(progman, IntPtr.Zero, "SHELLDLL_DefView", null);
        IntPtr w = IntPtr.Zero;
        while (view == IntPtr.Zero && (w = FindWindowExW(IntPtr.Zero, w, "WorkerW", null)) != IntPtr.Zero)
            view = FindWindowExW(w, IntPtr.Zero, "SHELLDLL_DefView", null);
        return view == IntPtr.Zero ? IntPtr.Zero : FindWindowExW(view, IntPtr.Zero, "SysListView32", "FolderView");
    }
    public static string State() {
        IntPtr t = Taskbar(), i = Icons();
        return "taskbar=" + (t != IntPtr.Zero && IsWindowVisible(t)) + " icons=" + (i != IntPtr.Zero && IsWindowVisible(i));
    }
    // The framed window's DWM frame (CaptureHelpers.GetWindowRectangle).
    public static RECT Frame(string title) {
        IntPtr h = FindWindowExW(IntPtr.Zero, IntPtr.Zero, null, title);
        RECT r; if (DwmGetWindowAttribute(h, 9, out r, 16) != 0) GetWindowRect(h, out r);
        return r;
    }
}
'@
# What Capture > Window lists, in order (as capture-session.ps1 has it).
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class WcsList {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    public static List<string> Titles() {
        var list = new List<string>();
        EnumWindows((h, l) => {
            if (!IsWindowVisible(h)) return true;
            int cloaked; if (DwmGetWindowAttribute(h, 14, out cloaked, 4) == 0 && cloaked != 0) return true;
            var t = new StringBuilder(512); GetWindowTextW(h, t, 512);
            if (t.Length == 0) return true;
            var c = new StringBuilder(256); GetClassNameW(h, c, 256);
            var cls = c.ToString();
            if (string.Equals(cls, "Progman", StringComparison.OrdinalIgnoreCase) || string.Equals(cls, "Button", StringComparison.OrdinalIgnoreCase)) return true;
            RECT r; GetWindowRect(h, out r);
            if (r.R - r.L <= 0 || r.B - r.T <= 0) return true;
            list.Add(t.ToString());
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
'@
$taskbar0 = [WcsWin]::IsWindowVisible([WcsWin]::Taskbar())
$icons0 = [WcsWin]::IsWindowVisible([WcsWin]::Icons())
"$(Get-Date -Format HH:mm:ss.fff) start: $([WcsWin]::State())"
$folder = if ($Process -eq 'ShareX') { 'G:/scripts/sharexrs-ref/ShareX-build/ShareX/Screenshots' } else { "$PSScriptRoot/../target/debug/ShareXrs/Screenshots" }
$before = @(Get-ChildItem -Recurse -File $folder -ErrorAction SilentlyContinue | ForEach-Object FullName)
$pattern = Start-Process -FilePath $Sharexrs -ArgumentList @('--test-pattern', '--seconds', '60', '--framed', ($Framed -join ',')) -PassThru
# Watches the two states from another runspace, every 2 ms.
$watch = [PowerShell]::Create()
[void]$watch.AddScript({
    param($stop, $log)
    $last = ''
    while (-not $stop.Value) {
        $s = [WcsWin]::State()
        if ($s -ne $last) { $log.Add("$(Get-Date -Format HH:mm:ss.fff) $s"); $last = $s }
        Start-Sleep -Milliseconds 2
    }
}).AddArgument(($stop = [ref]$false)).AddArgument(($log = [System.Collections.Concurrent.ConcurrentBag[string]]::new()))
try {
    & "$PSScriptRoot/wait-pattern.ps1" -Min 0.85
    $f = [WcsWin]::Frame('ShareXrs framed test window')
    "frame $($f.L),$($f.T) $($f.R - $f.L)x$($f.B - $f.T)"
    # The framed window in front: a click in its client area.
    & "$PSScriptRoot/input.ps1" -Click ($Framed[0] + [int]($Framed[2] / 2)), ($Framed[1] + [int]($Framed[3] / 2))
    Start-Sleep -Milliseconds 400
    $watch.Runspace = [runspacefactory]::CreateRunspace(); $watch.Runspace.Open()
    $handle = $watch.BeginInvoke()
    $exe = if ($Process -eq 'sharexrs') { $Sharexrs } else { $ShareXExe }
    "$(Get-Date -Format HH:mm:ss.fff) -$Job"
    if ($Job -eq 'Menu') {
        $title = 'ShareXrs framed test window'
        if ($Process -eq 'ShareX') {
            & "$PSScriptRoot/sharex-menu.ps1" -Process ShareX -Menu Capture -Entry Window, $title
        } else {
            # ShareXrs's menus: Capture at client (60, 60), its entries 28px
            # apart from y 66, a submenu's first 21px below its parent's top;
            # the Window submenu lists the windows in z-order.
            $client = (& "$PSScriptRoot/raise.ps1" -Process sharexrs -Title '^ShareXrs [0-9]') -split ','
            $ox = [int]$client[0]; $oy = [int]$client[1]
            & "$PSScriptRoot/input.ps1" -Click ($ox + 60), ($oy + 60)
            Start-Sleep -Milliseconds 1200
            $y = $oy + 66 + 28
            & "$PSScriptRoot/input.ps1" -MoveTo ($ox + 300), $y
            Start-Sleep -Milliseconds 1500
            $index = [WcsList]::Titles().IndexOf($title)
            if ($index -lt 0) { throw "'$title' isn't listed" }
            & "$PSScriptRoot/input.ps1" -Click ($ox + 580), ($y - 14 + 21 + 28 * $index)
        }
    } else {
        Start-Process -FilePath $exe -ArgumentList "-$Job" | Out-Null
    }
    if ($Job -eq 'RectangleRegion') {
        # The overlay, then a click on the window (it snaps to it).
        for ($i = 0; $i -lt 60; $i++) {
            $open = @(& "$PSScriptRoot/winrects.ps1" -Process $Process 2>$null | Where-Object { $_ -match '"Region capture"' })
            if ($open.Count -gt 0) { break }
            Start-Sleep -Milliseconds 500
        }
        Start-Sleep -Milliseconds 800
        # On the title bar: the window itself (the client area snaps as a control).
        & "$PSScriptRoot/input.ps1" -MoveTo ($Framed[0] + 100), ($Framed[1] + 14)
        Start-Sleep -Milliseconds 300
        & "$PSScriptRoot/input.ps1" -Click ($Framed[0] + 100), ($Framed[1] + 14)
    }
    # Up to -WaitMs for the file (a forwarded job is slow in a debug build).
    $deadline = (Get-Date).AddMilliseconds($WaitMs)
    do {
        Start-Sleep -Milliseconds 250
        $new = @(Get-ChildItem -Recurse -File $folder -ErrorAction SilentlyContinue | Where-Object { $before -notcontains $_.FullName })
    } while ($new.Count -eq 0 -and (Get-Date) -lt $deadline)
    Start-Sleep -Milliseconds 500
    $c = [WcsWin]::Client('ShareXrs framed test window')
    foreach ($n in $new) {
        # A client area capture starts at the client area, others at the frame.
        $check = python "$PSScriptRoot/pattern-check.py" $n.FullName --origin "$($c.X),$($c.Y)" --min $Min --opaque
        if ($LASTEXITCODE -ne 0 -or [double](($check | Select-String '([0-9.]+) of sampled').Matches[0].Groups[1].Value) -lt $Min) {
            $check = python "$PSScriptRoot/pattern-check.py" $n.FullName --origin "$($f.L),$($f.T)" --min $Min --opaque --search 30 --delete
        }
        "$($n.Name): $check"
        if (Test-Path $n.FullName) {
            if ($Out) { New-Item -ItemType Directory -Force $Out | Out-Null; Copy-Item $n.FullName (Join-Path $Out "$Job.png") }
        } else {
            Add-Type -AssemblyName System.Windows.Forms
            [System.Windows.Forms.Clipboard]::Clear()
            'clipboard cleared'
        }
    }
    if ($new.Count -eq 0) { 'no new file' }
}
finally {
    $stop.Value = $true
    Start-Sleep -Milliseconds 50
    if ($handle) { [void]$watch.EndInvoke($handle) }
    $watch.Dispose()
    $log | Sort-Object | ForEach-Object { "  watch $_" }
    Stop-Process -Id $pattern.Id -ErrorAction SilentlyContinue
    # Leave the taskbar and the icons as they were.
    $t = [WcsWin]::Taskbar(); $i = [WcsWin]::Icons()
    if ($t -ne [IntPtr]::Zero -and [WcsWin]::IsWindowVisible($t) -ne $taskbar0) { [void][WcsWin]::ShowWindow($t, $(if ($taskbar0) { 5 } else { 0 })); 'taskbar restored by the script' }
    if ($i -ne [IntPtr]::Zero -and [WcsWin]::IsWindowVisible($i) -ne $icons0) { [void][WcsWin]::ShowWindow($i, $(if ($icons0) { 5 } else { 0 })); 'icons restored by the script' }
    "$(Get-Date -Format HH:mm:ss.fff) end: $([WcsWin]::State())"
}
