<#
.SYNOPSIS
  ToolOnize environment setup wizard - Windows.
  ToolOnize is a CRMonize.com open-source project - https://crmonize.com

.DESCRIPTION
  Checks what ToolOnize needs, shows ONE plan, asks ONE consent, then elevates
  ONCE (a single UAC prompt) to install every machine-wide item with winget in
  one go. Rust (rustup) is per-user and is installed afterwards without
  elevation. See docs/adr/009-environment-setup-wizard.md.

  Works with Windows PowerShell 5.1 and PowerShell 7.

.PARAMETER SetupProfile
  dev (default): build from source. run: only what the built app needs.
.PARAMETER Check
  Report only; exit 0 when every required item is present, 1 otherwise.
.PARAMETER DryRun
  Show the plan; change nothing.
.PARAMETER Yes
  Accept the plan without prompting.
.PARAMETER NoOptional
  Do not offer optional tools (Git, OpenSSH client).

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\setup\windows.ps1
#>
[CmdletBinding()]
param(
  [ValidateSet('dev', 'run')][string]$SetupProfile = 'dev',
  [switch]$Check,
  [switch]$DryRun,
  [switch]$Yes,
  [switch]$NoOptional,
  # Internal: the elevated half of the install. Items are re-validated.
  [string]$ElevatedItems = ''
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$NodeMin = [version]'20.0.0'
$RustMin = [version]'1.77.2'
$WebView2Guid = '{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'

# The only things this script can install. Elevated runs accept nothing else.
$Catalog = [ordered]@{
  webview2   = @{ Label = 'Microsoft Edge WebView2 Runtime'; Kind = 'winget'; Id = 'Microsoft.EdgeWebView2Runtime'; Override = $null }
  buildtools = @{ Label = 'Visual Studio 2022 Build Tools (C++ workload)'; Kind = 'winget'; Id = 'Microsoft.VisualStudio.2022.BuildTools'; Override = '--wait --passive --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended' }
  node       = @{ Label = 'Node.js LTS'; Kind = 'winget'; Id = 'OpenJS.NodeJS.LTS'; Override = $null }
  git        = @{ Label = 'Git'; Kind = 'winget'; Id = 'Git.Git'; Override = $null }
  ssh        = @{ Label = 'OpenSSH client (Windows feature)'; Kind = 'capability'; Id = 'OpenSSH.Client~~~~0.0.1.0'; Override = $null }
  rust       = @{ Label = 'Rust via rustup (per user)'; Kind = 'user'; Id = 'Rustlang.Rustup'; Override = $null }
}

function Get-FirstVersion([string]$Text) {
  if ($Text -match '(\d+)\.(\d+)\.(\d+)') { return [version]"$($Matches[1]).$($Matches[2]).$($Matches[3])" }
  return $null
}

function Invoke-Quiet([string]$Exe, [string[]]$Arguments) {
  try {
    $out = & $Exe @Arguments 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    return ($out | Out-String)
  } catch { return $null }
}

function Test-WebView2 {
  $paths = @(
    "HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$WebView2Guid",
    "HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$WebView2Guid",
    "HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$WebView2Guid"
  )
  foreach ($p in $paths) {
    try {
      $pv = (Get-ItemProperty -Path $p -Name pv -ErrorAction Stop).pv
      if ($pv -and $pv -ne '0.0.0.0') { return $pv }
    } catch { }
  }
  return $null
}

function Test-MsvcTools {
  $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
  if (-not (Test-Path $vswhere)) { return $null }
  $path = Invoke-Quiet $vswhere @('-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath')
  if ($path) { return $path.Trim() }
  return $null
}

function Get-Requirements {
  $reqs = New-Object System.Collections.ArrayList
  function Add-Req($Id, $Label, $Level, $Ok, $Detail) {
    [void]$reqs.Add([pscustomobject]@{ Id = $Id; Label = $Label; Level = $Level; Ok = $Ok; Detail = $Detail })
  }

  $wv = Test-WebView2
  if ($wv) { Add-Req 'webview2' $Catalog.webview2.Label 'required' $true "version $wv" }
  else { Add-Req 'webview2' $Catalog.webview2.Label 'required' $false 'not installed' }

  if ($SetupProfile -eq 'dev') {
    $vs = Test-MsvcTools
    if ($vs) { Add-Req 'buildtools' 'MSVC C++ build tools' 'required' $true $vs }
    else { Add-Req 'buildtools' 'MSVC C++ build tools' 'required' $false 'not found' }

    $nv = Get-FirstVersion (Invoke-Quiet 'node' @('--version'))
    $npm = [bool](Get-Command npm -ErrorAction SilentlyContinue)
    if ($nv -and $nv -ge $NodeMin -and $npm) { Add-Req 'node' "Node.js >= $NodeMin and npm" 'required' $true "node $nv" }
    elseif ($nv) { Add-Req 'node' "Node.js >= $NodeMin and npm" 'required' $false "node $nv is too old or npm missing" }
    else { Add-Req 'node' "Node.js >= $NodeMin and npm" 'required' $false 'not found' }

    $rv = Get-FirstVersion (Invoke-Quiet 'rustc' @('--version'))
    $cargo = [bool](Get-Command cargo -ErrorAction SilentlyContinue)
    if ($rv -and $rv -ge $RustMin -and $cargo) { Add-Req 'rust' "Rust >= $RustMin (rustc, cargo)" 'required' $true "rustc $rv" }
    elseif ($rv) { Add-Req 'rust' "Rust >= $RustMin (rustc, cargo)" 'required' $false "rustc $rv is too old" }
    else { Add-Req 'rust' "Rust >= $RustMin (rustc, cargo)" 'required' $false 'not found' }
  }

  if (-not $NoOptional) {
    if (Get-Command ssh -ErrorAction SilentlyContinue) { Add-Req 'ssh' 'OpenSSH client (SSH members)' 'optional' $true 'found' }
    else { Add-Req 'ssh' 'OpenSSH client (SSH members)' 'optional' $false 'not found' }
    if ($SetupProfile -eq 'dev') {
      if (Get-Command git -ErrorAction SilentlyContinue) { Add-Req 'git' 'Git' 'optional' $true 'found' }
      else { Add-Req 'git' 'Git' 'optional' $false 'not found' }
    }
  }
  return , $reqs
}

function Format-Report($Reqs) {
  $lines = foreach ($r in $Reqs) {
    if ($r.Ok) { $mark = '[ ok ]    ' }
    elseif ($r.Level -eq 'required') { $mark = '[MISSING] ' }
    else { $mark = '[optional]' }
    "  $mark $($r.Label) - $($r.Detail)"
  }
  return ($lines -join [Environment]::NewLine)
}

function Test-Admin {
  $id = [Security.Principal.WindowsIdentity]::GetCurrent()
  return (New-Object Security.Principal.WindowsPrincipal($id)).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

function Install-Item([string]$Name) {
  $item = $Catalog[$Name]
  Write-Host "Installing $($item.Label)..."
  if ($item.Kind -eq 'capability') {
    Add-WindowsCapability -Online -Name $item.Id | Out-Null
    return $true
  }
  $wingetArgs = @('install', '--exact', '--id', $item.Id, '--silent', '--accept-package-agreements', '--accept-source-agreements', '--disable-interactivity')
  if ($item.Override) { $wingetArgs += @('--override', $item.Override) }
  & winget @wingetArgs | Out-Host
  # 0 = installed; -1978335189 (0x8A15002B) = already installed / no upgrade.
  return ($LASTEXITCODE -eq 0 -or $LASTEXITCODE -eq -1978335189)
}

function Wait-Next([string]$Title, [string]$Text) {
  Write-Host ''
  Write-Host "== $Title =="
  Write-Host $Text
  if ($Yes) { return }
  $answer = Read-Host 'Press Enter for Next (q to cancel)'
  if ($answer -eq 'q') { Write-Host 'Setup cancelled. Nothing was changed.'; exit 3 }
}

# ---------------------------------------------------------------- elevated half
if ($ElevatedItems) {
  $failed = @()
  foreach ($name in ($ElevatedItems -split ',')) {
    if (-not $Catalog.Contains($name) -or $Catalog[$name].Kind -eq 'user') { Write-Error "refusing unknown item: $name"; exit 2 }
    try { if (-not (Install-Item $name)) { $failed += $name } } catch { $failed += $name }
  }
  if ($failed.Count -gt 0) { Write-Host "Failed: $($failed -join ', ')"; exit 1 }
  exit 0
}

# ---------------------------------------------------------------------- main
$reqs = Get-Requirements
$report = Format-Report $reqs
$missingRequired = @($reqs | Where-Object { -not $_.Ok -and $_.Level -eq 'required' }).Count
$missingOptional = @($reqs | Where-Object { -not $_.Ok -and $_.Level -eq 'optional' }).Count
$header = "System: Windows $([Environment]::OSVersion.Version) (profile: $SetupProfile)"

if ($Check) {
  Write-Host $header
  Write-Host $report
  Write-Host ''
  Write-Host "Required missing: $missingRequired, optional missing: $missingOptional"
  if ($missingRequired -eq 0) { exit 0 } else { exit 1 }
}

function Get-Plan([bool]$IncludeOptional) {
  $todo = @($reqs | Where-Object { -not $_.Ok -and ($_.Level -eq 'required' -or $IncludeOptional) } | ForEach-Object { $_.Id })
  return , $todo
}

function Format-Plan($Todo) {
  if ($Todo.Count -eq 0) { return 'Nothing to install.' }
  $elevated = @($Todo | Where-Object { $Catalog[$_].Kind -ne 'user' })
  $lines = @()
  if ($elevated.Count -gt 0) {
    $lines += '-  One administrator authorization (UAC), then:'
    foreach ($n in $elevated) {
      if ($Catalog[$n].Kind -eq 'winget') { $lines += "     winget install --id $($Catalog[$n].Id)   ($($Catalog[$n].Label))" }
      else { $lines += "     Add-WindowsCapability -Online -Name $($Catalog[$n].Id)" }
    }
  }
  if ($Todo -contains 'rust') { $lines += '-  For your user (no elevation): winget install --id Rustlang.Rustup; rustup default stable-msvc' }
  return ($lines -join [Environment]::NewLine)
}

if ($DryRun) {
  Write-Host $header
  Write-Host $report
  Write-Host ''
  Write-Host 'Plan (dry run - nothing will be changed):'
  Write-Host (Format-Plan (Get-Plan (-not $NoOptional)))
  exit 0
}

Wait-Next 'Welcome' "This wizard prepares this PC for ToolOnize, a CRMonize.com open-source project.`n1. Check  2. One plan  3. One consent + one UAC prompt  4. Install and verify.`nNothing is changed until you accept the plan."
Wait-Next 'Check results' "$header`n$report"

if ($missingRequired -eq 0 -and $missingOptional -eq 0) { Write-Host 'Everything ToolOnize needs is already installed.'; exit 0 }

$includeOptional = $false
if ($missingOptional -gt 0) {
  if ($Yes) { $includeOptional = $true }
  else { $includeOptional = ((Read-Host 'Also install the missing optional tools (Git, OpenSSH client)? [Y/n]') -notmatch '^(n|no)$') }
}
$todo = Get-Plan $includeOptional
if ($todo.Count -eq 0) { Write-Host 'Nothing to install.'; exit 0 }

Write-Host ''
Write-Host '== Install plan =='
Write-Host (Format-Plan $todo)
if (-not $Yes) {
  if ((Read-Host 'Install? [y/N]') -notmatch '^(y|yes)$') { Write-Host 'Setup cancelled. Nothing was changed.'; exit 3 }
}

if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
  Write-Host 'winget is required. Install "App Installer" from the Microsoft Store, then rerun this wizard.'
  exit 4
}

$failed = @()
$elevated = @($todo | Where-Object { $Catalog[$_].Kind -ne 'user' })
if ($elevated.Count -gt 0) {
  $list = $elevated -join ','
  if (Test-Admin) {
    foreach ($n in $elevated) { try { if (-not (Install-Item $n)) { $failed += $n } } catch { $failed += $n } }
  } else {
    Write-Host 'Requesting administrator authorization once for all system installs...'
    $shell = (Get-Process -Id $PID).Path
    $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', "`"$PSCommandPath`"", '-ElevatedItems', $list)
    try {
      $p = Start-Process -FilePath $shell -ArgumentList $argList -Verb RunAs -Wait -PassThru
      if ($p.ExitCode -ne 0) { $failed += 'system-installs' }
    } catch {
      Write-Host 'Authorization was declined. Nothing else will be installed.'
      exit 3
    }
  }
}

if ($todo -contains 'rust') {
  try {
    if (-not (Install-Item 'rust')) { $failed += 'rust' }
    $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    & rustup default stable-msvc
    if ($LASTEXITCODE -ne 0) { $failed += 'rust-toolchain' }
  } catch { $failed += 'rust' }
}

# Pick up PATH changes made by the installers, then verify.
$env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
$reqs = Get-Requirements
$still = @($reqs | Where-Object { -not $_.Ok -and $_.Level -eq 'required' }).Count
Write-Host ''
Write-Host '== Finished =='
Write-Host (Format-Report $reqs)
if ($still -eq 0 -and $failed.Count -eq 0) {
  if ($SetupProfile -eq 'dev') { Write-Host "`nSetup complete. Open a new terminal, then: npm ci; npm run tauri -- dev" }
  else { Write-Host "`nSetup complete." }
  exit 0
}
Write-Host "`nSome steps did not complete: $($failed -join ', ')"
exit 1
