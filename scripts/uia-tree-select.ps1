<#
.SYNOPSIS
  Selects the tree item whose text is -Item in a window of a running app
  (Avalonia's TreeViewItems have no names of their own: their header text
  does), through UI Automation.

.EXAMPLE
  scripts/uia-tree-select.ps1 -Process ShareX -Window 'Destination settings' -Item 'Amazon S3'
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Window,
    [Parameter(Mandatory)] [string] $Item
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]
$ids = @(Get-Process -Name $Process | ForEach-Object Id)
$win = $AE::RootElement.FindAll($scope::Children, [System.Windows.Automation.Condition]::TrueCondition) |
    Where-Object { $ids -contains $_.Current.ProcessId -and $_.Current.Name -match $Window } | Select-Object -First 1
if (-not $win) { throw "no window of $Process matching '$Window'" }
$cond = New-Object System.Windows.Automation.AndCondition(
    (New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $Item)),
    (New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::Text)))
$text = $win.FindFirst($scope::Descendants, $cond)
if (-not $text) { throw "no item '$Item'" }
$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
$node = $walker.GetParent($text)
while ($node -and $node.Current.ControlType -ne [System.Windows.Automation.ControlType]::TreeItem) { $node = $walker.GetParent($node) }
if (-not $node) { throw "'$Item' isn't in a tree item" }
$pattern = $null
if ($node.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) { $pattern.Select() } else { throw "'$Item' can't be selected" }
"selected '$Item'"
