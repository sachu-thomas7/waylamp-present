param([string]$Click = '', [int]$Index = 0)
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$ae = [System.Windows.Automation.AutomationElement]
$root = $ae::RootElement
$proc = Get-Process streamdeck -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Output 'NO PROCESS'; exit 1 }
$cond = New-Object System.Windows.Automation.PropertyCondition($ae::ProcessIdProperty, $proc.Id)
$win = $root.FindAll('Children', $cond) | Where-Object { $_.Current.Name -like '*Controller*' } | Select-Object -First 1
if (-not $win) { Write-Output 'NO WINDOW'; exit 1 }
if ($Click) {
  $el = $win.FindAll('Descendants', [System.Windows.Automation.Condition]::TrueCondition) |
    Where-Object { $_.Current.Name -like "*$Click*" } | Select-Object -Skip $Index -First 1
  if (-not $el) { Write-Output "NOT FOUND: $Click"; exit 1 }
  $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
  Write-Output "CLICKED: $Click"
  exit 0
}
$all = $win.FindAll('Descendants', [System.Windows.Automation.Condition]::TrueCondition)
foreach ($e in $all) {
  $n = $e.Current.Name
  if ($n) { Write-Output ('{0} | {1}' -f $e.Current.ControlType.ProgrammaticName, $n) }
}
