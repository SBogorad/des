[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [string]$HostAlias,
  [Parameter(Mandatory = $true)]
  [string]$RemoteDir,
  [switch]$SkipBuild,
  [string]$RollbackReleaseId,
  [ValidateRange(1, 20)]
  [int]$KeepReleases = 3,
  [string]$SmokeUrl
)

$ErrorActionPreference = 'Stop'
$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$DistDir = Join-Path $ProjectDir 'dist'
$NodeExecutable = (Get-Command node -ErrorAction Stop).Source
$ReleaseRoot = "$RemoteDir/releases"
$CurrentLink = "$RemoteDir/current"

if ($HostAlias -notmatch '^[A-Za-z0-9_.@:-]+$') {
  throw 'HostAlias contains invalid characters.'
}
if ($RemoteDir -notmatch '^[A-Za-z0-9_./-]+$') {
  throw 'RemoteDir contains invalid characters.'
}

function Invoke-Native([string]$FilePath, [string[]]$Arguments) {
  & $FilePath @Arguments
  if ($LASTEXITCODE -ne 0) {
    throw "$FilePath failed with exit code $LASTEXITCODE"
  }
}

function Invoke-Remote([string]$Command) {
  Invoke-Native 'ssh' @($HostAlias, "set -eu; $Command")
}

function Test-HttpSecurityHeaders([string]$Url) {
  if (-not $Url) { return }
  $headers = & curl.exe --fail --silent --show-error --head $Url
  if ($LASTEXITCODE -ne 0) { throw "Smoke URL failed: $Url" }
  foreach ($required in @('content-security-policy:', 'x-content-type-options: nosniff')) {
    if (-not ($headers -join "`n").ToLowerInvariant().Contains($required)) {
      throw "Smoke URL is missing required security header: $required"
    }
  }
}

if ($RollbackReleaseId) {
  if ($RollbackReleaseId -notmatch '^[A-Za-z0-9._-]+$') {
    throw 'RollbackReleaseId contains invalid characters.'
  }
  Invoke-Remote "test -d '$ReleaseRoot/$RollbackReleaseId'; ln -s '$ReleaseRoot/$RollbackReleaseId' '$CurrentLink.next'; mv -Tf '$CurrentLink.next' '$CurrentLink'"
  Invoke-Remote "test -s '$CurrentLink/index.html'; test -s '$CurrentLink/demo/findings.json'"
  Test-HttpSecurityHeaders $SmokeUrl
  Write-Host "Rolled back DES site to $RollbackReleaseId." -ForegroundColor Green
  exit 0
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

$gitCommit = (& git -C $ProjectDir rev-parse --short=12 HEAD).Trim()
$releaseId = "$(Get-Date -AsUTC -Format 'yyyyMMddHHmmss')-$gitCommit"
$manifestPath = Join-Path $DistDir 'RELEASE-MANIFEST.txt'
$manifestLines = Get-ChildItem -LiteralPath $DistDir -Recurse -File |
  Where-Object { $_.FullName -ne $manifestPath } |
  Sort-Object FullName |
  ForEach-Object {
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash.ToLowerInvariant()
    $relative = [IO.Path]::GetRelativePath($DistDir, $_.FullName).Replace('\', '/')
    "$hash  $relative"
  }
Set-Content -LiteralPath $manifestPath -Value $manifestLines -Encoding ascii

Invoke-Remote "mkdir -p '$ReleaseRoot/$releaseId'"
Invoke-Native 'scp' @('-r', (Join-Path $DistDir '*'), "${HostAlias}:$ReleaseRoot/$releaseId/")
Invoke-Remote "cd '$ReleaseRoot/$releaseId'; sha256sum -c RELEASE-MANIFEST.txt; test -s index.html; test -s demo/findings.json; ln -s '$ReleaseRoot/$releaseId' '$CurrentLink.next'; mv -Tf '$CurrentLink.next' '$CurrentLink'; find '$ReleaseRoot' -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | sort -r | tail -n +$($KeepReleases + 1) | xargs -r -I{} rm -rf '$ReleaseRoot/{}'"
Invoke-Remote "test -s '$CurrentLink/index.html'; test -s '$CurrentLink/demo/findings.json'; printf 'release_id=$releaseId\n'; readlink '$CurrentLink'"
Test-HttpSecurityHeaders $SmokeUrl

Remove-Item -LiteralPath $manifestPath -Force
Write-Host "DES site deployed atomically as $releaseId." -ForegroundColor Green
