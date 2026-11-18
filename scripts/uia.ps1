<#
.SYNOPSIS
  Drives a running app through UI Automation, for side-by-side checks of
  ShareX and ShareXrs: list a window's elements, or invoke/click one by name.

.EXAMPLE
  scripts/uia.ps1 -Process ShareX -Window 'Hotkey' -Invoke 'OK'
  scripts/uia.ps1 -Process ShareX -Window '^ShareX' -Dump | Select-Object -First 80
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [string] $Window = '.',
    [string] $Invoke,
    # A regex on the element's name instead of the exact name.
    [string] $InvokeMatch,
    [switch] $Dump
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]

$ids = @(Get-Process -Name $Process | ForEach-Object Id)
$win = $AE::RootElement.FindAll($scope::Children, [System.Windows.Automation.Condition]::TrueCondition) |
    Where-Object { $ids -contains $_.Current.ProcessId -and $_.Current.Name -match $Window } |
    Select-Object -First 1
if (-not $win) {
    # Some windows (topmost dialogs) aren't among the root's children:
    # find them by enumerating top-level windows instead.
    Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class UiaWin {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
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
    $pidSet = New-Object 'System.Collections.Generic.HashSet[uint32]'
    $ids | ForEach-Object { [void]$pidSet.Add([uint32]$_) }
    $hit = [UiaWin]::Windows($pidSet) | Where-Object { $_.Item2 -match $Window } | Select-Object -First 1
    if ($hit) { $win = $AE::FromHandle($hit.Item1) }
}
if (-not $win) { throw "no window of $Process matching '$Window'" }

if ($Dump) {
    $win.FindAll($scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object {
        $c = $_.Current
        '{0,-14} {1,-40} {2}' -f $c.ControlType.ProgrammaticName.Replace('ControlType.', ''), $c.Name, $c.AutomationId
    }
    return
}

if ($InvokeMatch) {
    # The first element whose name matches the regex (names with a tooltip's
    # second line, such as the editor's "Image Effects").
    $el = $win.FindAll($scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
        Where-Object { $_.Current.Name -match $InvokeMatch } | Select-Object -First 1
    if (-not $el) { throw "no element matching '$InvokeMatch'" }
    $Invoke = $el.Current.Name
}

if ($Invoke) {
    if (-not $el) {
        $el = $win.FindFirst($scope::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $Invoke)))
    }
    if (-not $el) { throw "no element named '$Invoke'" }
    $pattern = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { $pattern.Invoke() }
    elseif ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) { $pattern.Select() }
    elseif ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) { $pattern.Toggle() }
    elseif ($el.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) { $pattern.Expand() }
    else { throw "'$Invoke' has no invoke/select/toggle/expand pattern" }
    "invoked '$Invoke'"
}
