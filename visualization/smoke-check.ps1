[CmdletBinding()]
param(
  [string]$BaseUrl = 'https://ced.syntog.ru',
  [switch]$SkipApiCheck
)

$ErrorActionPreference = 'Stop'
$PresentationUrl = "$BaseUrl/viz/?crew=CREW-0004&mode=presentation"
$ApiUrl = "$BaseUrl/api/v1/health/live"

function Get-CheckedResponse([string]$Url) {
  try {
    return Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 30
  } catch {
    throw "Request failed: $Url`n$($_.Exception.Message)"
  }
}

$presentation = Get-CheckedResponse $PresentationUrl
if ($presentation.StatusCode -ne 200) {
  throw "Presentation URL returned HTTP $($presentation.StatusCode)"
}

$mainAsset = [regex]::Match($presentation.Content, 'src="(/viz/assets/index-[^"]+\.js)"').Groups[1].Value
if (-not $mainAsset) {
  throw 'Presentation HTML does not reference a /viz main application asset.'
}

$asset = Get-CheckedResponse "$BaseUrl$mainAsset"
$network = Get-CheckedResponse "$BaseUrl/viz/network.json"
$simulation = Get-CheckedResponse "$BaseUrl/viz/sim.json"
$findings = Get-CheckedResponse "$BaseUrl/viz/findings.json"
$api = if (-not $SkipApiCheck) { Get-CheckedResponse $ApiUrl }

if ($asset.StatusCode -ne 200 -or $asset.Content.Length -lt 1000) {
  throw 'Main application asset is missing or unexpectedly small.'
}
if ($network.StatusCode -ne 200 -or $network.Content.Length -lt 1000) {
  throw 'Network dataset is missing or unexpectedly small.'
}
if ($simulation.StatusCode -ne 200 -or $simulation.Content.Length -lt 1000) {
  throw 'Simulation dataset is missing or unexpectedly small.'
}
if ($findings.StatusCode -ne 200 -or $findings.Content.Length -lt 100) {
  throw 'Findings contract is missing or unexpectedly small.'
}
$parsedFindings = $findings.Content | ConvertFrom-Json
if (-not ($parsedFindings | Where-Object { $_.crewId -eq 'CREW-0004' -and $_.severity -eq 'critical' })) {
  throw 'Critical CREW-0004 finding is missing from the findings contract.'
}
if ($api -and $api.StatusCode -ne 200) {
  throw "CED API health returned HTTP $($api.StatusCode)"
}

$results = [ordered]@{
  Presentation = "$($presentation.StatusCode) $PresentationUrl"
  MainAsset = "$($asset.StatusCode) $mainAsset ($($asset.Content.Length) bytes)"
  Network = "$($network.StatusCode) /viz/network.json ($($network.Content.Length) bytes)"
  Simulation = "$($simulation.StatusCode) /viz/sim.json ($($simulation.Content.Length) bytes)"
  Findings = "$($findings.StatusCode) /viz/findings.json ($($findings.Content.Length) bytes)"
}
if ($api) {
  $results.Api = "$($api.StatusCode) /api/v1/health/live"
}
[PSCustomObject]$results | Format-List

Write-Host 'Production smoke check passed.' -ForegroundColor Green
