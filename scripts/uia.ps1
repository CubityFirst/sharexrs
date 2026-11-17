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
if (-not $win) { throw "no window of $Process matching '$Window'" }

if ($Dump) {
    $win.FindAll($scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object {
        $c = $_.Current
        '{0,-14} {1,-40} {2}' -f $c.ControlType.ProgrammaticName.Replace('ControlType.', ''), $c.Name, $c.AutomationId
    }
    return
}

if ($Invoke) {
    $el = $win.FindFirst($scope::Descendants, (New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $Invoke)))
    if (-not $el) { throw "no element named '$Invoke'" }
    $pattern = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { $pattern.Invoke() }
    elseif ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) { $pattern.Select() }
    elseif ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$pattern)) { $pattern.Toggle() }
    elseif ($el.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) { $pattern.Expand() }
    else { throw "'$Invoke' has no invoke/select/toggle/expand pattern" }
    "invoked '$Invoke'"
}
