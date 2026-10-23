# Regression cases for unsafe portable ZIP entry names, executed on Windows CI.
# Build ZIP files in memory so forbidden Win32 names never touch the filesystem.
$ErrorActionPreference = 'Stop'
$checker = Join-Path $PSScriptRoot '..\check-windows-cli.ps1'
$version = '9.9.9'
$target = 'x86_64-pc-windows-msvc'
$folder = "repodna-$version-$target"
$root = Join-Path ([System.IO.Path]::GetTempPath()) (
    'repodna-invalid-paths-' + [System.Guid]::NewGuid().ToString('N')
)
New-Item -ItemType Directory -Path $root -Force | Out-Null

try {
    foreach ($badPath in @(
        'CON.txt', 'COM¹.txt', 'LPT²', 'trailing.', 'trailing ',
        'file:stream', 'bad|name', 'bad<name', 'bad>name', 'bad?name',
        'bad*name', 'bad"name', 'nested//file.txt', 'nested/../outside.txt'
    )) {
        $file = Join-Path $root ([System.Guid]::NewGuid().ToString('N') + '.zip')
        $archive = [System.IO.Compression.ZipFile]::Open(
            $file, [System.IO.Compression.ZipArchiveMode]::Create
        )
        try {
            $exe = $archive.CreateEntry("$folder/repodna.exe")
            $exeStream = $exe.Open()
            try {
                $bytes = [byte[]]@(0x4D, 0x5A, 0x00, 0x00)
                $exeStream.Write($bytes, 0, $bytes.Length)
            }
            finally { $exeStream.Dispose() }
            $null = $archive.CreateEntry("$folder/$badPath")
        }
        finally { $archive.Dispose() }

        $rejected = $false
        try {
            & $checker -Archive $file -Version $version -Target $target -SkipExecution | Out-Null
        }
        catch {
            if ($_.Exception.Message -notmatch 'Unsafe or duplicate ZIP entry') {
                throw ("Wrong validation error for " + $badPath + ": " + $_.Exception.Message)
            }
            $rejected = $true
        }
        if (-not $rejected) {
            throw "Accepted an unsafe ZIP path: $badPath"
        }
    }
    Write-Output 'All unsafe Windows ZIP path regression tests passed.'
}
finally {
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
