[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$HostAlias,
  [Parameter(Mandatory = $true)]
  [string]$RemoteDir,
  [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DistDir = Join-Path $ProjectDir 'dist'
$NodeExecutable = (Get-Command node -ErrorAction Stop).Source

function Invoke-Native([string]$FilePath, [string[]]$Arguments) {
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE"
  }
}

if (-not $SkipBuild) {
  Push-Location $ProjectDir
  try {
    Invoke-Native $NodeExecutable @('build-site.mjs')
  } finally {
    Pop-Location
  }
}

foreach ($required in @('index.html', 'styles.css', 'demo/index.html', 'demo/findings.json')) {
  if (-not (Test-Path -LiteralPath (Join-Path $DistDir $required))) {
    throw "Build output is missing: $required"
  }
}

Invoke-Native 'ssh' @($HostAlias, "set -e; mkdir -p $RemoteDir")
Invoke-Native 'scp' @('-r', (Join-Path $DistDir '*'), "${HostAlias}:$RemoteDir/")
Invoke-Native 'ssh' @($HostAlias, "set -e; grep -q 'Discrete-event simulation' $RemoteDir/index.html; test -f $RemoteDir/demo/findings.json; echo deployed:$RemoteDir")

Write-Host 'DES site deployed successfully.' -ForegroundColor Green
Write-Host 'Verify the configured deployment URL separately.'
