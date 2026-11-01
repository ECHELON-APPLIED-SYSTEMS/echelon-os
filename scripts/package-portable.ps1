param([string]$Executable, [string]$Readme, [string]$Output)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-RegularPortableFile([string]$Path) {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
    if ($item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
        ($item.Attributes -band [IO.FileAttributes]::Hidden) -or ($item.Attributes -band [IO.FileAttributes]::System)) {
        throw 'Portable payload must contain regular visible files only'
    }
    return $item
}

function Assert-PortablePayload([string]$Root, [long]$MaxBytes = 50MB) {
    $rootItem = Get-Item -LiteralPath $Root -Force -ErrorAction Stop
    if (-not $rootItem.PSIsContainer -or ($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw 'Unsafe portable staging root'
    }
    # Deliberately no recursion: ANY child directory or link fails before it can be traversed.
    $entries = @(Get-ChildItem -LiteralPath $Root -Force -ErrorAction Stop)
    if ($entries.Count -ne 2) { throw 'Unexpected portable payload entries' }
    $names = @('echelon-os.exe', 'README.txt')
    [long]$bytes = 0
    foreach ($entry in $entries) {
        if (-not ($names -ccontains $entry.Name)) { throw 'Unexpected portable payload name' }
        $file = Assert-RegularPortableFile $entry.FullName
        $bytes += $file.Length
    }
    if ($bytes -gt $MaxBytes) { throw 'Portable payload exceeds uncompressed size limit' }
}

function Assert-PortableArchive([string]$Path, [long]$MaxBytes = 50MB) {
    $zip = [IO.Compression.ZipFile]::OpenRead($Path)
    try {
        $expected = @('ECHELON-OS/echelon-os.exe', 'ECHELON-OS/README.txt')
        $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
        [long]$bytes = 0
        foreach ($entry in $zip.Entries) {
            if ((($entry.ExternalAttributes -shr 16) -band 0xF000) -eq 0xA000 -or
                ($entry.ExternalAttributes -band 0x400)) { throw 'Linked archive entry' }
            if (-not $seen.Add($entry.FullName)) { throw 'Duplicate archive entry' }
            if ($entry.FullName -ceq 'ECHELON-OS/' -and $entry.Length -eq 0) { continue }
            if (-not ($expected -ccontains $entry.FullName)) { throw 'Unexpected archive entry' }
            $unixType = ($entry.ExternalAttributes -shr 16) -band 0xF000
            if (($entry.ExternalAttributes -band 0x16) -or ($unixType -ne 0 -and $unixType -ne 0x8000)) {
                throw 'Archive payload is not a regular visible file'
            }
            $bytes += $entry.Length
        }
        foreach ($name in $expected) {
            if (-not $seen.Contains($name)) { throw 'Missing archive entry' }
        }
        if ($bytes -gt $MaxBytes) { throw 'Archive exceeds uncompressed size limit' }
    } finally { $zip.Dispose() }
}

function Invoke-PortablePackage([string]$Executable, [string]$Readme, [string]$Output) {
    if (-not $Executable -or -not $Readme -or -not $Output) { throw 'Executable, readme and output are required' }
    $exe = Assert-RegularPortableFile $Executable
    $guide = Assert-RegularPortableFile $Readme
    $destination = [IO.Path]::GetFullPath($Output)
    if (Test-Path -LiteralPath $destination) { throw 'Refusing to replace existing archive' }
    $parent = [IO.Path]::GetDirectoryName($destination)
    [IO.Directory]::CreateDirectory($parent) | Out-Null
    $parentItem = Get-Item -LiteralPath $parent -Force
    if ($parentItem.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked output directory' }
    # Own only this newly created scratch folder; never clean arbitrary existing staging/output data.
    $scratch = Join-Path $parent ('.portable-stage-' + [Guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $scratch -ErrorAction Stop | Out-Null
    try {
        $stage = Join-Path $scratch 'ECHELON-OS'
        New-Item -ItemType Directory -Path $stage -ErrorAction Stop | Out-Null
        Copy-Item -LiteralPath $exe.FullName -Destination (Join-Path $stage 'echelon-os.exe') -ErrorAction Stop
        Copy-Item -LiteralPath $guide.FullName -Destination (Join-Path $stage 'README.txt') -ErrorAction Stop
        Assert-PortablePayload $stage
        $archive = Join-Path $scratch 'portable.zip'
        [IO.Compression.ZipFile]::CreateFromDirectory($stage, $archive, [IO.Compression.CompressionLevel]::Optimal, $true)
        Assert-PortableArchive $archive
        # Same-parent no-replace publication; a concurrent existing destination also fails.
        [IO.File]::Move($archive, $destination)
    } finally { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction Stop }
}

if ($MyInvocation.InvocationName -ne '.') {
    Invoke-PortablePackage $Executable $Readme $Output
}
