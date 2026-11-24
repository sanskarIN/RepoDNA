# Offline preflight for one architecture's unpublished v1.5.3 Windows trial files.
# Verifies the two package hashes against the matching GitHub Actions
# trial-sha256sums artifact, then checks NSIS setup EXE version metadata.
# This script does not install or run the application.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('x64', 'arm64')]
    [string] $Architecture,
    [Parameter(Mandatory = $true)]
    [string] $Directory,
    [Parameter(Mandatory = $true)]
    [string] $ChecksumFile,
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version = '1.5.3'
)

$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $Directory -PathType Container)) {
    throw "Trial installer directory does not exist: $Directory"
}
if (-not (Test-Path -LiteralPath $ChecksumFile -PathType Leaf)) {
    throw "Trial SHA256SUMS.txt is missing: $ChecksumFile"
}

$hashes = @{}
foreach ($line in Get-Content -LiteralPath $ChecksumFile) {
    if ($line -match '^([0-9a-fA-F]{64})\s+\*?(.+)
    if ($name -match '[/\\]' -or $name -match '^\s|\s$') {
        throw "Unexpected manifest filename: $name"
    }
    if ($hashes.ContainsKey($name)) {
        throw "Duplicate checksum manifest filename: $name"
    }
    $hashes[$name] = $hash
}
if ($hashes.Count -ne 15) {
    throw "Unexpected trial manifest size: $($hashes.Count) checksums; expected 15."
}

$files = @(
    "RepoDNA_${Version}_${Architecture}_en-US.msi",
    "RepoDNA_${Version}_${Architecture}-setup.exe"
)
foreach ($name in $files) {
    if (-not $hashes.ContainsKey($name)) {
        throw "Required installer is absent from SHA256SUMS.txt: $name"
    }
    $path = Join-Path $Directory $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required installer is missing: $path"
    }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -cne $hashes[$name]) {
        throw "SHA-256 mismatch for ${name}: expected $($hashes[$name]); found $actual"
    }
    Write-Host "Verified SHA-256: $name"
}

$metadataChecker = Join-Path $PSScriptRoot '..\desktop\check-installer-metadata.ps1'
if (-not (Test-Path -LiteralPath $metadataChecker -PathType Leaf)) {
    throw "RepoDNA NSIS metadata checker is missing from this checkout: $metadataChecker"
}
& $metadataChecker -Directory $Directory -Version $Version -Architecture $Architecture
Write-Host "PASS: RepoDNA $Version $Architecture trial package preflight."
Write-Host "Native clean installation, upgrade, launch and uninstall still require physical Windows QA."
) {
        $hash = $Matches[1].ToLowerInvariant()
        $name = $Matches[2]
    } else {
        throw "Invalid checksum manifest entry: $line"
    }
    if ($name -match '[/\\]' -or $name -match '^\s|\s$') {
        throw "Unexpected manifest filename: $name"
    }
    if ($hashes.ContainsKey($name)) {
        throw "Duplicate checksum manifest filename: $name"
    }
    $hashes[$name] = $hash
}
if ($hashes.Count -ne 15) {
    throw "Unexpected trial manifest size: $($hashes.Count) checksums; expected 15."
}

$files = @(
    "RepoDNA_${Version}_${Architecture}_en-US.msi",
    "RepoDNA_${Version}_${Architecture}-setup.exe"
)
foreach ($name in $files) {
    if (-not $hashes.ContainsKey($name)) {
        throw "Required installer is absent from SHA256SUMS.txt: $name"
    }
    $path = Join-Path $Directory $name
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
        throw "Required installer is missing: $path"
    }
    $actual = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -cne $hashes[$name]) {
        throw "SHA-256 mismatch for ${name}: expected $($hashes[$name]); found $actual"
    }
    Write-Host "Verified SHA-256: $name"
}

$metadataChecker = Join-Path $PSScriptRoot '..\desktop\check-installer-metadata.ps1'
if (-not (Test-Path -LiteralPath $metadataChecker -PathType Leaf)) {
    throw "RepoDNA NSIS metadata checker is missing from this checkout: $metadataChecker"
}
& $metadataChecker -Directory $Directory -Version $Version -Architecture $Architecture
Write-Host "PASS: RepoDNA $Version $Architecture trial package preflight."
Write-Host "Native clean installation, upgrade, launch and uninstall still require physical Windows QA."
