<#
.SYNOPSIS
  Lists the topmost visible top-level windows, top first: handle, process,
  title, rectangle and whether DWM cloaks it. For checking which window an
  overlay ended up under.
#>
param([int] $Count = 6)
Add-Type @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class ZOrder {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int a, out int v, int s);
    public struct RECT { public int L, T, R, B; }
}
'@
$list = New-Object System.Collections.ArrayList
$cb = [ZOrder+EnumProc] {
    param($h, $l)
    if ([ZOrder]::IsWindowVisible($h)) {
        $cloaked = 0
        [void][ZOrder]::DwmGetWindowAttribute($h, 14, [ref]$cloaked, 4)
        if ($cloaked -eq 0) { [void]$list.Add($h) }
    }
    return $list.Count -lt $Count
}
[void][ZOrder]::EnumWindows($cb, [IntPtr]::Zero)
foreach ($h in $list) {
    $sb = New-Object System.Text.StringBuilder 256
    [void][ZOrder]::GetWindowText($h, $sb, 256)
    $procId = 0
    [void][ZOrder]::GetWindowThreadProcessId($h, [ref]$procId)
    $r = New-Object ZOrder+RECT
    [void][ZOrder]::GetWindowRect($h, [ref]$r)
    $name = (Get-Process -Id $procId -ErrorAction SilentlyContinue).ProcessName
    "$h $name($procId) '$($sb.ToString())' $($r.L),$($r.T) $($r.R - $r.L)x$($r.B - $r.T)"
}
