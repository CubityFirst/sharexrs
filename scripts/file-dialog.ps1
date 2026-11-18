<#
.SYNOPSIS
  Answers a running app's open/save/folder dialog (the Windows common file
  dialog, class #32770): puts the paths into its "File name" (or "Folder")
  box and presses Open/Save/Select Folder, or Cancel. For feeding the same
  synthetic files to ShareX's and ShareXrs's tool windows.

.EXAMPLE
  scripts/file-dialog.ps1 -Process ShareX -Files G:\in\a.png,G:\in\b.png
  scripts/file-dialog.ps1 -Process sharexrs -Folder G:\out
  scripts/file-dialog.ps1 -Process ShareX -Cancel
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [string[]] $Files,
    [string] $Folder,
    [switch] $Cancel,
    [int] $TimeoutMs = 8000
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class FileDlg {
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr parent, IntPtr after, string cls, string title);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, string l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc f, IntPtr l);
    [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
    [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr h);
    /// A descendant Edit with control id 1001 (the save dialog's file name).
    public static IntPtr EditById(IntPtr dlg, int id) {
        IntPtr hit = IntPtr.Zero;
        EnumChildWindows(dlg, (h, l) => {
            var sb = new StringBuilder(64); GetClassNameW(h, sb, 64);
            if (sb.ToString() == "Edit" && GetDlgCtrlID(h) == id) { hit = h; return false; }
            return true;
        }, IntPtr.Zero);
        return hit;
    }
    /// The Edit of the first ComboBox among the dialog's descendants (the
    /// save dialog keeps its file name box deep in its view).
    public static IntPtr ComboEdit(IntPtr dlg) {
        IntPtr hit = IntPtr.Zero;
        EnumChildWindows(dlg, (h, l) => {
            var sb = new StringBuilder(64); GetClassNameW(h, sb, 64);
            if (sb.ToString() == "Edit") {
                var pb = new StringBuilder(64); GetClassNameW(GetParent(h), pb, 64);
                if (pb.ToString() == "ComboBox") { hit = h; return false; }
            }
            return true;
        }, IntPtr.Zero);
        return hit;
    }
    public static IntPtr Find(HashSet<uint> pids) {
        IntPtr hit = IntPtr.Zero;
        EnumWindows((h, l) => {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pids.Contains(pid) && IsWindowVisible(h)) {
                var sb = new StringBuilder(64); GetClassNameW(h, sb, 64);
                if (sb.ToString() == "#32770") { hit = h; return false; }
            }
            return true;
        }, IntPtr.Zero);
        return hit;
    }
}
'@
$pids = New-Object 'System.Collections.Generic.HashSet[uint32]'
Get-Process -Name $Process | ForEach-Object { [void]$pids.Add([uint32]$_.Id) }

$deadline = [DateTime]::Now.AddMilliseconds($TimeoutMs)
$dlg = [IntPtr]::Zero
while ($dlg -eq [IntPtr]::Zero -and [DateTime]::Now -lt $deadline) {
    $dlg = [FileDlg]::Find($pids)
    if ($dlg -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 150 }
}
if ($dlg -eq [IntPtr]::Zero) { throw "no file dialog of $Process" }
# Let the dialog finish building its controls.
Start-Sleep -Milliseconds 400

$WM_SETTEXT = 0x000C
$BM_CLICK = 0x00F5
function Press([int] $Id) {
    $b = [FileDlg]::GetDlgItem($dlg, $Id)
    if ($b -eq [IntPtr]::Zero) { throw "no button $Id" }
    [void][FileDlg]::PostMessageW($b, $BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
}

if ($Cancel) {
    Press 2
    'cancelled'
    return
}

$text = if ($Folder) { $Folder } elseif ($Files.Count -eq 1) { $Files[0] } else { ($Files | ForEach-Object { '"' + $_ + '"' }) -join ' ' }
# Open dialogs: the ComboBoxEx32 1148 with its Edit; save dialogs and the
# folder picker keep an Edit inside a ComboBox (1001 / 1152 in some forms).
$edit = [IntPtr]::Zero
$combo = [FileDlg]::GetDlgItem($dlg, 1148)
if ($combo -ne [IntPtr]::Zero) {
    $inner = [FileDlg]::FindWindowExW($combo, [IntPtr]::Zero, 'ComboBox', $null)
    $edit = [FileDlg]::FindWindowExW($(if ($inner -ne [IntPtr]::Zero) { $inner } else { $combo }), [IntPtr]::Zero, 'Edit', $null)
}
if ($edit -eq [IntPtr]::Zero) {
    foreach ($id in 1152, 1001) {
        $c = [FileDlg]::GetDlgItem($dlg, $id)
        if ($c -eq [IntPtr]::Zero) { continue }
        $edit = [FileDlg]::FindWindowExW($c, [IntPtr]::Zero, 'Edit', $null)
        if ($edit -eq [IntPtr]::Zero) { $edit = $c }
        break
    }
}
if ($edit -eq [IntPtr]::Zero) { $edit = [FileDlg]::EditById($dlg, 1001) }
if ($edit -eq [IntPtr]::Zero) {
    # The Vista save dialog: the file name box is a DirectUI host
    # (FileNameControlHost); set it through UI Automation.
    Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
    $AE = [System.Windows.Automation.AutomationElement]
    $root = $AE::FromHandle($dlg)
    $host0 = $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, 'FileNameControlHost')))
    if (-not $host0) { throw 'no file name box' }
    $editCond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)
    $box = $host0.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $editCond)
    if (-not $box) { $box = $host0 }
    $box.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($text)
} else {
    [void][FileDlg]::SendMessageW($edit, $WM_SETTEXT, [IntPtr]::Zero, $text)
}
Start-Sleep -Milliseconds 150
Press 1
"answered: $text"
