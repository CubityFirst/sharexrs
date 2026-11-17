<#
.SYNOPSIS
  Selects pages of a settings window's navigation (Application settings,
  Task settings: `SettingsNavigation`, a TreeView) through UI Automation and
  screenshots each, for comparing ShareX and ShareXrs page by page.

.EXAMPLE
  scripts/settings-pages.ps1 -Process ShareX -Window 'Application settings' -OutDir shots/sharex/appsettings -Suffix '-dark'
  scripts/settings-pages.ps1 -Process ShareX -Window 'Application settings' -Page 1   # just select Theme

  Pages are named after their navigation text, lower-cased with spaces as
  dashes (`main-window-dark.png`). -Page selects one page (by index) and
  captures it only when -OutDir is given.
#>
param(
    [Parameter(Mandatory)] [string] $Process,
    [string] $Window = 'Application settings',
    [string] $OutDir,
    [string] $Suffix = '',
    [int] $Page = -1
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
$cond = New-Object System.Windows.Automation.PropertyCondition($AE::ControlTypeProperty, [System.Windows.Automation.ControlType]::TreeItem)
$items = @($win.FindAll($scope::Descendants, $cond))
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
for ($i = 0; $i -lt $items.Count; $i++) {
    if ($Page -ge 0 -and $i -ne $Page) { continue }
    $item = $items[$i]
    $name = $item.Current.Name
    if (-not $name) {
        $text = $item.FindFirst($scope::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
        if ($text) { $name = $text.Current.Name }
    }
    $pattern = $null
    if ($item.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$pattern)) { $pattern.Select() }
    else { throw "page $i ($name) can't be selected" }
    "selected $i $name"
    if ($OutDir) {
        Start-Sleep -Milliseconds 600
        $file = Join-Path $OutDir (($name.ToLowerInvariant() -replace '\s+', '-') + "$Suffix.png")
        & "$here\winshot.ps1" -Process $Process -Title $Window -Out $file
    }
}
