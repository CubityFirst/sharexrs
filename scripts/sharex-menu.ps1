<#
.SYNOPSIS
  Runs a sidebar menu entry of a running ShareX (or ShareXrs) through UI
  Automation: invokes the sidebar button, then the entry in the flyout it
  opens (Avalonia puts the flyout in its own top-level window).

.EXAMPLE
  scripts/sharex-menu.ps1 -Process ShareX -Menu Capture -Entry "Region..."
  # A submenu's entry: each name in turn.
  scripts/sharex-menu.ps1 -Process ShareX -Menu Capture -Entry "Monitor","1. 1024x768"
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [Parameter(Mandatory)] [string] $Menu,
    [Parameter(Mandatory)] [string[]] $Entry
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$AE = [System.Windows.Automation.AutomationElement]
$scope = [System.Windows.Automation.TreeScope]
$ids = @(Get-Process -Name $Process | ForEach-Object Id)

function Find-Named([string] $Name) {
    $windows = @($AE::RootElement.FindAll($scope::Children, [System.Windows.Automation.Condition]::TrueCondition) |
        Where-Object { $ids -contains $_.Current.ProcessId })
    $condition = New-Object System.Windows.Automation.PropertyCondition($AE::NameProperty, $Name)
    foreach ($w in $windows) {
        $found = $w.FindFirst($scope::Descendants, $condition)
        if ($found) { return ,$found }
    }
    return $null
}
function Invoke-Element([System.Windows.Automation.AutomationElement] $Element) {
    $pattern = $null
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern, [ref]$pattern)) { $pattern.Invoke(); return }
    if ($Element.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern, [ref]$pattern)) { $pattern.Expand(); return }
    throw "'$($Element.Current.Name)' can't be invoked"
}

$menuElement = Find-Named $Menu
if (-not $menuElement) { throw "no '$Menu' in $Process" }
Invoke-Element $menuElement
foreach ($name in $Entry) {
    $entryElement = $null
    for ($i = 0; $i -lt 20 -and -not $entryElement; $i++) {
        Start-Sleep -Milliseconds 100
        $entryElement = Find-Named $name
    }
    if (-not $entryElement) { throw "no '$name' after opening '$Menu'" }
    Invoke-Element $entryElement
}
"invoked $Menu > $($Entry -join ' > ')"
