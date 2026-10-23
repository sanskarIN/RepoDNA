# Validate a RepoDNA Windows portable ZIP before publishing or submitting it to WinGet.
# Checks file layout, duplicate/unsafe paths, architecture, and (when runnable)
# the executable's behavior without installing anything or requiring elevation.
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string] $Archive,
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^\d+\.\d+\.\d+$')]
    [string] $Version,
    [Parameter(Mandatory = $true)]
    [ValidateSet('x86_64-pc-windows-msvc', 'aarch64-pc-windows-msvc')]
    [string] $Target,
    [switch] $SkipExecution
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$folder = "repodna-$Version-$Target"
$executablePath = "$folder/repodna.exe"
$resolvedArchive = (Resolve-Path -LiteralPath $Archive).Path
$archiveFile = [System.IO.Compression.ZipFile]::OpenRead($resolvedArchive)
try {
    $seen = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::OrdinalIgnoreCase
    )
    $exeCount = 0
    foreach ($entry in $archiveFile.Entries) {
        $name = $entry.FullName
        $normalized = $name.Replace('\', '/')
        if (
            ($normalized -cne $name) -or
            (-not $normalized.StartsWith("$folder/", [System.StringComparison]::Ordinal)) -or
            (-not $seen.Add($normalized))
        ) {
            throw "Unsafe or duplicate ZIP entry: $name"
        }
        # Match the WinGet generator's checks before Windows extracts the archive.
        # Directory entries have one trailing slash, which is not a filename segment.
        $segments = @($normalized.Split('/'))
        if ($normalized.EndsWith('/')) {
            $segments = @($segments[0..($segments.Length - 2)])
        }
        foreach ($segment in $segments) {
            if (
                (-not $segment) -or
                ($segment -eq '.') -or
                ($segment -eq '..') -or
                ($segment -match '[\x00-\x1f\x7f<>:"|?*]') -or
                ($segment -match '[. ]
        $unixType = ($entry.ExternalAttributes -shr 16) -band 0xF000
        if ($unixType -eq 0xA000) {
            throw "Symlink entry is not permitted: $name"
        }
        if ($name -ceq $executablePath) {
            if ($entry.Length -eq 0) { throw "Portable executable is empty: $name" }
            $exeCount += 1
        }
    }
    if ($exeCount -ne 1) {
        throw "Expected exactly one $executablePath; found $exeCount"
    }
}
finally {
    $archiveFile.Dispose()
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    "repodna-winget-check-" + [System.Guid]::NewGuid().ToString('N')
)
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    [System.IO.Compression.ZipFile]::ExtractToDirectory($resolvedArchive, $temporaryRoot)
    $exe = Join-Path (Join-Path $temporaryRoot $folder) 'repodna.exe'
    $stream = [System.IO.File]::OpenRead($exe)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 64 -or $peOffset -gt ($stream.Length - 6)) {
            throw 'Portable executable has an invalid PE header offset'
        }
        $stream.Position = $peOffset
        if ([System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(4)) -cne "PE$([char]0)$([char]0)") {
            throw 'Portable executable does not contain a valid PE signature'
        }
        $machine = $reader.ReadUInt16()
        $expectedMachine = if ($Target -eq 'x86_64-pc-windows-msvc') { 0x8664 } else { 0xAA64 }
        if ($machine -ne $expectedMachine) {
            throw ("Incorrect PE architecture: expected 0x{0:X4}, found 0x{1:X4}" -f $expectedMachine, $machine)
        }
    }
    finally {
        $reader.Dispose()
    }

    if (-not $SkipExecution) {
        # Isolate all checks from actual user configuration and storage.
        $oldHome = $env:REPODNA_HOME
        $env:REPODNA_HOME = Join-Path $temporaryRoot 'repodna-home'
        try {
            $output = (& $exe --version 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch [regex]::Escape($Version)) {
                throw 'Portable executable --version failed'
            }
            $output = (& $exe --help 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch 'Usage:') {
                throw 'Portable executable --help failed'
            }
            $output = (& $exe 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch 'Usage:') {
                throw 'Portable executable with no arguments failed'
            }
            $null = (& $exe __repodna_invalid_smoke_command__ 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 2) {
                throw 'Invalid CLI arguments must exit with usage code 2'
            }
        }
        finally {
            $env:REPODNA_HOME = $oldHome
        }
    }
    # The deliberately invalid command above exits 2 as expected. PowerShell
    # must not propagate that expected native exit code as the CI step status.
    $global:LASTEXITCODE = 0
    Write-Output "Validated $folder portable ZIP (architecture, layout, and CLI launch checks)."
}
finally {
    Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
}
) -or
                ($segment -match '^(?:con|prn|aux|nul|com[1-9¹²³]|lpt[1-9¹²³])(?:\..*)?
        $unixType = ($entry.ExternalAttributes -shr 16) -band 0xF000
        if ($unixType -eq 0xA000) {
            throw "Symlink entry is not permitted: $name"
        }
        if ($name -ceq $executablePath) {
            if ($entry.Length -eq 0) { throw "Portable executable is empty: $name" }
            $exeCount += 1
        }
    }
    if ($exeCount -ne 1) {
        throw "Expected exactly one $executablePath; found $exeCount"
    }
}
finally {
    $archiveFile.Dispose()
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    "repodna-winget-check-" + [System.Guid]::NewGuid().ToString('N')
)
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    [System.IO.Compression.ZipFile]::ExtractToDirectory($resolvedArchive, $temporaryRoot)
    $exe = Join-Path (Join-Path $temporaryRoot $folder) 'repodna.exe'
    $stream = [System.IO.File]::OpenRead($exe)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 64 -or $peOffset -gt ($stream.Length - 6)) {
            throw 'Portable executable has an invalid PE header offset'
        }
        $stream.Position = $peOffset
        if ([System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(4)) -cne "PE$([char]0)$([char]0)") {
            throw 'Portable executable does not contain a valid PE signature'
        }
        $machine = $reader.ReadUInt16()
        $expectedMachine = if ($Target -eq 'x86_64-pc-windows-msvc') { 0x8664 } else { 0xAA64 }
        if ($machine -ne $expectedMachine) {
            throw ("Incorrect PE architecture: expected 0x{0:X4}, found 0x{1:X4}" -f $expectedMachine, $machine)
        }
    }
    finally {
        $reader.Dispose()
    }

    if (-not $SkipExecution) {
        # Isolate all checks from actual user configuration and storage.
        $oldHome = $env:REPODNA_HOME
        $env:REPODNA_HOME = Join-Path $temporaryRoot 'repodna-home'
        try {
            $output = (& $exe --version 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch [regex]::Escape($Version)) {
                throw 'Portable executable --version failed'
            }
            $output = (& $exe --help 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch 'Usage:') {
                throw 'Portable executable --help failed'
            }
            $output = (& $exe 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch 'Usage:') {
                throw 'Portable executable with no arguments failed'
            }
            $null = (& $exe __repodna_invalid_smoke_command__ 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 2) {
                throw 'Invalid CLI arguments must exit with usage code 2'
            }
        }
        finally {
            $env:REPODNA_HOME = $oldHome
        }
    }
    # The deliberately invalid command above exits 2 as expected. PowerShell
    # must not propagate that expected native exit code as the CI step status.
    $global:LASTEXITCODE = 0
    Write-Output "Validated $folder portable ZIP (architecture, layout, and CLI launch checks)."
}
finally {
    Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
}
)
            ) {
                throw "Unsafe or duplicate ZIP entry: $name"
            }
        }
        # Unix symbolic links can redirect an extracted executable outside its folder.
        $unixType = ($entry.ExternalAttributes -shr 16) -band 0xF000
        if ($unixType -eq 0xA000) {
            throw "Symlink entry is not permitted: $name"
        }
        if ($name -ceq $executablePath) {
            if ($entry.Length -eq 0) { throw "Portable executable is empty: $name" }
            $exeCount += 1
        }
    }
    if ($exeCount -ne 1) {
        throw "Expected exactly one $executablePath; found $exeCount"
    }
}
finally {
    $archiveFile.Dispose()
}

$temporaryRoot = Join-Path ([System.IO.Path]::GetTempPath()) (
    "repodna-winget-check-" + [System.Guid]::NewGuid().ToString('N')
)
New-Item -ItemType Directory -Path $temporaryRoot | Out-Null
try {
    [System.IO.Compression.ZipFile]::ExtractToDirectory($resolvedArchive, $temporaryRoot)
    $exe = Join-Path (Join-Path $temporaryRoot $folder) 'repodna.exe'
    $stream = [System.IO.File]::OpenRead($exe)
    $reader = [System.IO.BinaryReader]::new($stream)
    try {
        $stream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 64 -or $peOffset -gt ($stream.Length - 6)) {
            throw 'Portable executable has an invalid PE header offset'
        }
        $stream.Position = $peOffset
        if ([System.Text.Encoding]::ASCII.GetString($reader.ReadBytes(4)) -cne "PE$([char]0)$([char]0)") {
            throw 'Portable executable does not contain a valid PE signature'
        }
        $machine = $reader.ReadUInt16()
        $expectedMachine = if ($Target -eq 'x86_64-pc-windows-msvc') { 0x8664 } else { 0xAA64 }
        if ($machine -ne $expectedMachine) {
            throw ("Incorrect PE architecture: expected 0x{0:X4}, found 0x{1:X4}" -f $expectedMachine, $machine)
        }
    }
    finally {
        $reader.Dispose()
    }

    if (-not $SkipExecution) {
        # Isolate all checks from actual user configuration and storage.
        $oldHome = $env:REPODNA_HOME
        $env:REPODNA_HOME = Join-Path $temporaryRoot 'repodna-home'
        try {
            $output = (& $exe --version 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch [regex]::Escape($Version)) {
                throw 'Portable executable --version failed'
            }
            $output = (& $exe --help 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch 'Usage:') {
                throw 'Portable executable --help failed'
            }
            $output = (& $exe 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 0 -or $output -notmatch 'Usage:') {
                throw 'Portable executable with no arguments failed'
            }
            $null = (& $exe __repodna_invalid_smoke_command__ 2>&1 | Out-String)
            if ($LASTEXITCODE -ne 2) {
                throw 'Invalid CLI arguments must exit with usage code 2'
            }
        }
        finally {
            $env:REPODNA_HOME = $oldHome
        }
    }
    # The deliberately invalid command above exits 2 as expected. PowerShell
    # must not propagate that expected native exit code as the CI step status.
    $global:LASTEXITCODE = 0
    Write-Output "Validated $folder portable ZIP (architecture, layout, and CLI launch checks)."
}
finally {
    Remove-Item -LiteralPath $temporaryRoot -Recurse -Force -ErrorAction SilentlyContinue
}
