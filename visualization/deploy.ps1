[CmdletBinding()]
param(
  [string]$HostAlias = 'stroy',
  [string]$RemoteDir = '/opt/ced/viz',
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
    $env:BASE = '/viz/'
    Invoke-Native $NodeExecutable @('generate-findings.mjs')
    Invoke-Native $NodeExecutable @('node_modules/typescript/bin/tsc')
    Invoke-Native $NodeExecutable @('node_modules/vite/bin/vite.js', 'build')
  } finally {
    Pop-Location
  }
}

$IndexPath = Join-Path $DistDir 'index.html'
if (-not (Test-Path -LiteralPath $IndexPath)) {
  throw "Build output is missing: $IndexPath"
}

$index = Get-Content -LiteralPath $IndexPath -Raw
if ($index -notmatch 'src="/viz/assets/index-[^"]+\.js"') {
  throw 'dist/index.html is not built for the /viz/ deployment path.'
}

$mainAsset = [regex]::Match($index, 'src="(/viz/assets/index-[^"]+\.js)"').Groups[1].Value.Split('/')[-1]

Invoke-Native 'ssh' @($HostAlias, "set -e; mkdir -p $RemoteDir/assets")
Invoke-Native 'scp' @(
  (Join-Path $DistDir 'index.html'),
  (Join-Path $DistDir 'network.json'),
  (Join-Path $DistDir 'sim.json'),
  (Join-Path $DistDir 'findings.json'),
  "${HostAlias}:$RemoteDir/"
)
Invoke-Native 'scp' @('-r', (Join-Path $DistDir 'assets/*'), "${HostAlias}:$RemoteDir/assets/")
Invoke-Native 'ssh' @(
  $HostAlias,
  "set -e; grep -q '$mainAsset' $RemoteDir/index.html; test -f $RemoteDir/assets/$mainAsset; echo deployed:$mainAsset"
)

Write-Host 'Demo deployed successfully.' -ForegroundColor Green
Write-Host 'Verify: https://ced.syntog.ru/viz/?crew=CREW-0004&mode=presentation'
