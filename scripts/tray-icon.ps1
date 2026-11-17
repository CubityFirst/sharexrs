<#
.SYNOPSIS
  Finds an app's icon in the notification area (the taskbar's, else the
  overflow flyout's, which it opens) through UI Automation, and prints the
  icon's centre in physical screen pixels as "x,y", for input.ps1 to click.

.EXAMPLE
  $at = scripts/tray-icon.ps1 -Name '^ShareX$'
  scripts/input.ps1 -RightClick ($at -split ',')
  scripts/tray-icon.ps1 -Name . -List    # every icon's name and place
#>
param(
    # A regex matched against the icon's name (its tooltip).
    [string] $Name = '.',
    [switch] $List
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class TrayDpi {
    [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
}
'@
[void][TrayDpi]::SetProcessDpiAwarenessContext([IntPtr](-4))
$AE = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]
$true_ = [System.Windows.Automation.Condition]::TrueCondition

function Icons($root) {
    $root.FindAll($scope::Descendants, $true_) | Where-Object {
        $c = $_.Current
        $c.ControlType.ProgrammaticName -match 'Button|MenuItem' -and $c.Name -and
            ($c.AutomationId -match 'NotifyItemIcon|SystemTrayIcon' -or $c.ClassName -match 'SystemTray|NotifyIcon')
    }
}

$cond = New-Object System.Windows.Automation.PropertyCondition($AE::ClassNameProperty, 'Shell_TrayWnd')
$taskbar = $AE::RootElement.FindFirst($scope::Children, $cond)
$icons = @(Icons $taskbar)
$match = $icons | Where-Object { $_.Current.Name -match $Name } | Select-Object -First 1
if (-not $match -and -not $List) {
    # Hidden icons live in the overflow flyout, opened by the chevron.
    $chevron = $taskbar.FindAll($scope::Descendants, $true_) |
        Where-Object { $_.Current.AutomationId -match 'NotificationAreaOverflow|SystemTrayIcon' -and $_.Current.Name -match 'Hidden' } |
        Select-Object -First 1
    $overflowOpen = { $AE::RootElement.FindAll($scope::Children, $true_) | Where-Object { $_.Current.ClassName -match 'TopLevelWindowForOverflowXamlIsland|NotifyIconOverflowWindow' } }
    if ($chevron -and -not (& $overflowOpen)) {
        # A synthesized click doesn't always open it; invoking does.
        $chevron.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
        Start-Sleep -Milliseconds 1000
    }
    if ($chevron) {
        $overflow = $AE::RootElement.FindAll($scope::Children, $true_) |
            Where-Object { $_.Current.ClassName -match 'TopLevelWindowForOverflowXamlIsland|NotifyIconOverflowWindow' } |
            Select-Object -First 1
        if ($overflow) {
            $icons += @(Icons $overflow)
            $match = $icons | Where-Object { $_.Current.Name -match $Name } | Select-Object -First 1
        }
    }
}
if ($List) {
    $icons | ForEach-Object {
        $r = $_.Current.BoundingRectangle
        '{0,-40} {1},{2} {3}x{4}  {5}' -f $_.Current.Name, $r.X, $r.Y, $r.Width, $r.Height, $_.Current.AutomationId
    }
    return
}
if (-not $match) { throw "no notification area icon matching '$Name'" }
$r = $match.Current.BoundingRectangle
'{0},{1}' -f [int]($r.X + $r.Width / 2), [int]($r.Y + $r.Height / 2)
