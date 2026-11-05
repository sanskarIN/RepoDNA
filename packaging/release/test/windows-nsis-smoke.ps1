param(
  [Parameter(Mandatory = $true)]
  [string] $Installer
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$installerPath = (Resolve-Path -LiteralPath $Installer).Path

function Get-RepoDNAInstallations {
  $paths = @(
    'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\*',
    'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*'
  )
  @(Get-ItemProperty -Path $paths -ErrorAction SilentlyContinue |
    Where-Object { $_.PSObject.Properties['DisplayName'] -and $_.PSObject.Properties['DisplayName'].Value -ceq 'RepoDNA' })
}

if (@(Get-RepoDNAInstallations).Count -ne 0) {
  throw 'A RepoDNA installation was left behind by the MSI qualification.'
}

$process = Start-Process -FilePath $installerPath -ArgumentList '/S' -Wait -PassThru
if ($process.ExitCode -notin @(0, 3010)) {
  throw "Silent NSIS installation failed ($($process.ExitCode))."
}
Start-Sleep -Seconds 2

$entries = @(Get-RepoDNAInstallations)
if ($entries.Count -ne 1) {
  throw "Expected a single NSIS RepoDNA installation; found $($entries.Count)."
}
$entry = $entries[0]
if ($entry.DisplayVersion -cne '1.5.0') {
  throw "NSIS registered the wrong version: '$($entry.DisplayVersion)'."
}
$publisher = $entry.PSObject.Properties['Publisher']
if ($publisher -and $publisher.Value -and $publisher.Value -cne 'RepoDNA') {
  throw "NSIS publisher is '$($publisher.Value)', expected RepoDNA."
}

$uninstallProperty = $entry.PSObject.Properties['UninstallString']
if (-not $uninstallProperty -or -not $uninstallProperty.Value) {
  throw 'NSIS did not register an uninstall command.'
}
$uninstallCommand = [string] $uninstallProperty.Value
if ($uninstallCommand -match '^"([^"]+)"(.*)$') {
  $uninstaller = $Matches[1]
  $existingArgs = $Matches[2].Trim()
} elseif ($uninstallCommand -match '^(.*?\.exe)(.*)$') {
  $uninstaller = $Matches[1].Trim()
  $existingArgs = $Matches[2].Trim()
} else {
  throw 'Unrecognized NSIS uninstall command format.'
}
if (-not (Test-Path -LiteralPath $uninstaller -PathType Leaf)) {
  throw "NSIS uninstaller is missing: $uninstaller"
}
$appExe = Join-Path (Split-Path -Parent $uninstaller) 'repodna-desktop.exe'
if (-not (Test-Path -LiteralPath $appExe -PathType Leaf)) {
  throw "Installed NSIS executable missing: $appExe"
}
if ((Get-Item -LiteralPath $appExe).VersionInfo.CompanyName -cne 'RepoDNA') {
  throw 'Installed NSIS executable does not have CompanyName = RepoDNA.'
}
Write-Host 'Verified fresh NSIS installation, version 1.5.0 and CompanyName = RepoDNA.'

$uninstallArgs = ($existingArgs + ' /S').Trim()
$uninstall = Start-Process -FilePath $uninstaller -ArgumentList $uninstallArgs -Wait -PassThru
if ($uninstall.ExitCode -notin @(0, 3010)) {
  throw "NSIS uninstall failed ($($uninstall.ExitCode))."
}
for ($attempt = 1; $attempt -le 20; $attempt++) {
  if (@(Get-RepoDNAInstallations).Count -eq 0) {
    Write-Host 'Verified NSIS uninstall and registration cleanup.'
    exit 0
  }
  Start-Sleep -Seconds 1
}
throw 'NSIS uninstall left RepoDNA registered.'
