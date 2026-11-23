# Check the version resource of the *setup EXE* (not the installed app binary).
# Windows Explorer's Company field reads CompanyName from this file.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Directory,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version,
    [Parameter(Mandatory = $true)]
    [ValidateSet('x64', 'arm64')]
    [string] $Architecture
)

$ErrorActionPreference = 'Stop'
$installer = Join-Path $Directory "RepoDNA_${Version}_${Architecture}-setup.exe"
if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) {
    throw "RepoDNA $Architecture NSIS setup EXE is missing: $installer"
}

$info = (Get-Item -LiteralPath $installer).VersionInfo
$expected = [ordered]@{
    CompanyName = 'RepoDNA'
    ProductName = 'RepoDNA'
    FileDescription = 'RepoDNA'
    FileVersion = $Version
    ProductVersion = $Version
}

foreach ($field in $expected.Keys) {
    $actual = [string] $info.$field
    $value = [string] $expected[$field]
    # The OS may normalize a three-part version to four numeric components.
    $matchesVersion = ($field -in @('FileVersion', 'ProductVersion')) -and
        ($actual -ceq "$value.0")
    if (($actual -cne $value) -and (-not $matchesVersion)) {
        throw ("Invalid NSIS {0} in {1}: '{2}' (expected '{3}')" -f $field, $installer, $actual, $value)
    }
}
Write-Host "Verified RepoDNA $Version $Architecture NSIS installer version metadata: $installer"
