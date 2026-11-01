# Synthetic payloads only: never invokes cargo, Tauri, a Windows EXE or a workflow.
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/package-portable.ps1"
$root = Join-Path ([IO.Path]::GetTempPath()) ('echelon-package-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$script:checks = 0
function Check([string]$Name, [scriptblock]$Body) {
    & $Body
    $script:checks++
    Write-Output "PASS $Name"
}
function Reject([scriptblock]$Body) {
    $rejected = $false
    try { & $Body } catch { $rejected = $true }
    if (-not $rejected) { throw 'Expected rejection did not occur' }
}
function Make-Archive([string]$Path, [string[]]$Names) {
    $zip = [IO.Compression.ZipFile]::Open($Path, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $Names) {
            $entry = $zip.CreateEntry($name)
            if (-not $name.EndsWith('/')) {
                $stream = $entry.Open()
                try { $stream.WriteByte(65) } finally { $stream.Dispose() }
            }
        }
    } finally { $zip.Dispose() }
}
try {
    $exe = Join-Path $root 'synthetic.exe'
    $readme = Join-Path $root 'guide.txt'
    [IO.File]::WriteAllText($exe, 'NOT A WINDOWS EXECUTABLE')
    [IO.File]::WriteAllText($readme, 'Synthetic portable packaging test')
    $output = Join-Path $root 'out/portable.zip'
    Check 'clean packaging and exact output entries' {
        Invoke-PortablePackage $exe $readme $output
        Assert-PortableArchive $output
        $zip = [IO.Compression.ZipFile]::OpenRead($output)
        try {
            if ($zip.Entries.Count -ne 2) { throw 'Unexpected output count' }
            $stream = [IO.StreamReader]::new($zip.GetEntry('ECHELON-OS/echelon-os.exe').Open())
            try { if ($stream.ReadToEnd() -cne 'NOT A WINDOWS EXECUTABLE') { throw 'Wrong packaged bytes' } }
            finally { $stream.Dispose() }
        } finally { $zip.Dispose() }
        if (@(Get-ChildItem (Split-Path $output) -Force).Count -ne 1) { throw 'Staging was not cleaned' }
    }
    Check 'existing output is preserved' {
        $before = (Get-FileHash $output).Hash
        Reject { Invoke-PortablePackage $exe $readme $output }
        if ((Get-FileHash $output).Hash -cne $before) { throw 'Archive overwritten' }
    }
    $stage = Join-Path $root 'stage'
    New-Item -ItemType Directory -Path $stage | Out-Null
    Copy-Item $exe (Join-Path $stage 'echelon-os.exe')
    Copy-Item $readme (Join-Path $stage 'README.txt')
    Check 'empty extra directory rejected' {
        $extra = Join-Path $stage 'empty'
        New-Item -ItemType Directory -Path $extra | Out-Null
        Reject { Assert-PortablePayload $stage }
        Remove-Item $extra
    }
    Check 'hidden extra file rejected' {
        $extra = Join-Path $stage '.hidden'
        [IO.File]::WriteAllText($extra, 'hidden')
        Reject { Assert-PortablePayload $stage }
        Remove-Item -LiteralPath $extra -Force
    }
    Check 'staged symlink rejected without traversal' {
        $link = Join-Path $stage 'echelon-os.exe'
        Remove-Item $link
        New-Item -ItemType SymbolicLink -Path $link -Target $exe | Out-Null
        Reject { Assert-PortablePayload $stage }
        Remove-Item -LiteralPath $link -Force
        Copy-Item $exe $link
    }
    Check 'uncompressed stage size enforced' { Reject { Assert-PortablePayload $stage 1 } }
    $good = @('ECHELON-OS/echelon-os.exe', 'ECHELON-OS/README.txt')
    foreach ($bad in @('ECHELON-OS/empty/', 'ECHELON-OS/.hidden', '../escape', 'ECHELON-OS/app-data/settings.json', 'ECHELON-OS\README.txt', 'ECHELON-OS/echelon-os.exe')) {
        $path = Join-Path $root ([Guid]::NewGuid().ToString('N') + '.zip')
        Make-Archive $path ($good + $bad)
        Check "archive rejects $bad" { Reject { Assert-PortableArchive $path } }
    }
    Check 'missing archive file rejected' {
        $path = Join-Path $root 'missing.zip'
        Make-Archive $path @('ECHELON-OS/echelon-os.exe')
        Reject { Assert-PortableArchive $path }
    }
    Check 'harmless parent entry accepted' {
        $path = Join-Path $root 'parent.zip'
        Make-Archive $path ($good + 'ECHELON-OS/')
        Assert-PortableArchive $path
    }
    Check 'archive uncompressed size enforced' { Reject { Assert-PortableArchive $output 1 } }
    Check 'archive symlink attribute rejected' {
        $path = Join-Path $root 'link.zip'
        Make-Archive $path $good
        $zip = [IO.Compression.ZipFile]::Open($path, [IO.Compression.ZipArchiveMode]::Update)
        try { $zip.Entries[0].ExternalAttributes = [int](-1577123840) } finally { $zip.Dispose() } # Unix symlink mode 0xA1FF0000
        Reject { Assert-PortableArchive $path }
    }
    foreach ($attribute in @(0x10, 0x2, 0x10000000)) {
        Check "nonregular/hidden archive attribute $attribute rejected" {
            $path = Join-Path $root ([Guid]::NewGuid().ToString('N') + '.zip')
            Make-Archive $path $good
            $zip = [IO.Compression.ZipFile]::Open($path, [IO.Compression.ZipArchiveMode]::Update)
            try { $zip.Entries[0].ExternalAttributes = $attribute } finally { $zip.Dispose() }
            Reject { Assert-PortableArchive $path }
        }
    }
    Write-Output "$script:checks synthetic packaging checks passed. No application build performed."
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
