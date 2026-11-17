<#
.SYNOPSIS
  Closes one top-level window of a running process as its close button
  would (WM_CLOSE), found by a regex on its title.

.EXAMPLE
  scripts/close-window.ps1 -Process ShareX -Title 'Destination settings'
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Title
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class CloseWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    public static List<Tuple<IntPtr,string>> Windows(HashSet<uint> pids) {
        var list = new List<Tuple<IntPtr,string>>();
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pids.Contains(pid) && IsWindowVisible(h)) {
                var sb = new StringBuilder(512); GetWindowTextW(h, sb, 512);
                list.Add(Tuple.Create(h, sb.ToString()));
            }
            return true;
        }, IntPtr.Zero);
        return list;
    }
}
'@
$pids = New-Object 'System.Collections.Generic.HashSet[uint32]'
Get-Process -Name $Process | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }
$w = [CloseWin]::Windows($pids) | Where-Object { $_.Item2 -match $Title } | Select-Object -First 1
if (-not $w) { throw "no window of $Process matching '$Title'" }
[void][CloseWin]::PostMessageW($w.Item1, 0x10, [IntPtr]::Zero, [IntPtr]::Zero)
"closed '$($w.Item2)'"
