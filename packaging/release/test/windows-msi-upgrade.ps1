param(
  [Parameter(Mandatory = $true)]
  [string] $CandidateMsi
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$baselineVersion = '1.4.5'
$candidateVersion = '1.5.0'
$targetMsi = (Resolve-Path -LiteralPath $CandidateMsi).Path
$baselineMsi = Join-Path $env:RUNNER_TEMP 'RepoDNA_1.4.5_x64_en-US.msi'
$baselineUrl = "https://github.com/sanskarIN/RepoDNA/releases/download/v1.4.5/RepoDNA_1.4.5_x64_en-US.msi"

$downloaded = $false
for ($attempt = 1; $attempt -le 5; $attempt++) {
  try {
    Invoke-WebRequest -Uri $baselineUrl -OutFile $baselineMsi -MaximumRedirection 10
    if ((Get-Item -LiteralPath $baselineMsi).Length -lt 1000000) {
      throw 'The baseline MSI is unexpectedly small.'
    }
    $downloaded = $true
    break
  } catch {
    if ($attempt -eq 5) { throw }
    Start-Sleep -Seconds (5 * $attempt)
  }
}
if (-not $downloaded) { throw 'Unable to download published v1.4.5 MSI.' }

function Get-RepoDNAInstallations {
  $paths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
  )
  @(Get-ItemProperty -Path $paths -ErrorAction SilentlyContinue |
      Where-Object { $_.PSObject.Properties['DisplayName'] -and $_.PSObject.Properties['DisplayName'].Value -ceq 'RepoDNA' })
}

function Assert-OneVersion([string] $expected) {
  $installed = @(Get-RepoDNAInstallations)
  if ($installed.Count -ne 1) {
    throw "Expected one RepoDNA installation at version $expected, found $($installed.Count)."
  }
  if ($installed[0].DisplayVersion -cne $expected) {
    throw "Expected installed version $expected; found '$($installed[0].DisplayVersion)'."
  }
  return $installed[0]
}

function Invoke-Installer([string] $verb, [string] $msi, [string] $phase) {
  $log = Join-Path $env:RUNNER_TEMP "repodna-$phase.log"
  $arguments = "/$verb `"$msi`" /qn /norestart /l*v `"$log`""
  $process = Start-Process -FilePath msiexec.exe -ArgumentList $arguments -Wait -PassThru
  if ($process.ExitCode -notin @(0, 3010)) {
    Get-Content -LiteralPath $log -Tail 60 -ErrorAction SilentlyContinue | Write-Host
    throw "MSI $phase failed with exit code $($process.ExitCode)."
  }
}

# Check the installed Windows Add/Remove Programs entry, not only artifact filenames.
Invoke-Installer 'i' $baselineMsi 'baseline-install'
[void] (Assert-OneVersion $baselineVersion)

Invoke-Installer 'i' $targetMsi 'candidate-upgrade'
$newInstallation = Assert-OneVersion $candidateVersion
$publisher = $newInstallation.PSObject.Properties['Publisher']
if ($publisher -and $publisher.Value -and $publisher.Value -cne 'RepoDNA') {
  throw "Installed MSI publisher is '$($publisher.Value)', expected RepoDNA."
}
Write-Host "Verified in-place MSI upgrade $baselineVersion -> $candidateVersion, with one installed RepoDNA entry."

Invoke-Installer 'x' $targetMsi 'candidate-uninstall'
$remaining = @(Get-RepoDNAInstallations)
if ($remaining.Count -ne 0) {
  throw "RepoDNA remains registered after uninstall ($($remaining.Count) entries)."
}
Write-Host "Verified MSI uninstall after upgrade."

# Also verify the new MSI can install on a clean Windows runner, independently
# of the earlier version's UpgradeCode and registry history.
Invoke-Installer 'i' $targetMsi 'candidate-clean-install'
[void] (Assert-OneVersion $candidateVersion)
Invoke-Installer 'x' $targetMsi 'candidate-clean-uninstall'
if (@(Get-RepoDNAInstallations).Count -ne 0) {
  throw 'RepoDNA remains registered after clean-install uninstall.'
}
Write-Host "Verified clean v1.5.0 MSI installation and uninstall."
