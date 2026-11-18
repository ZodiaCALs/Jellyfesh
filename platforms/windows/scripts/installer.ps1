<#
.SYNOPSIS
Makes the Windows installer of Jellyfesh, Jellyfesh-<version>-Windows-Setup.exe, with Inno Setup.

.DESCRIPTION
Run it after scripts/build.ps1 has published both architectures, which leaves the apps in
platforms/windows/publish/win-x64/Jellyfesh and publish/win-arm64/Jellyfesh. It compiles
installer/Jellyfesh.iss with the Inno Setup command-line compiler (ISCC, Inno Setup 6.3 or later;
the windows-2025 runner image has one). The result is one Setup for x64 and Arm64 PCs that installs
the matching build for the current user, without administrator rights, with a Start menu shortcut
and an uninstaller, over any earlier version (docs/windows.md).

With -SigningCertificate, Setup and its uninstaller are signed through Inno Setup's Sign Tool with
signtool. The certificate is put in the current user's certificate store while ISCC runs and is
used by its thumbprint, so its password (WINDOWS_SIGNING_PASSWORD) is never on a command line, which
ISCC prints.

.PARAMETER Version
The app version for the file name and Settings > Apps. Default: the tag in GITHUB_REF_NAME (v0.0.7
gives 0.0.7), else the Version in Directory.Build.props.

.PARAMETER OutDir
Where Setup goes. Default: platforms/windows/artifacts.

.PARAMETER SigningCertificate
A .pfx code-signing certificate; its password is read from WINDOWS_SIGNING_PASSWORD.

.PARAMETER TimestampServer
The RFC 3161 timestamp server for -SigningCertificate. Default: http://timestamp.digicert.com.

.PARAMETER Website
The website shown for Jellyfesh in Settings > Apps (optional).
#>
[CmdletBinding()]
param(
    [string] $Version,

    [string] $OutDir,

    [string] $SigningCertificate,

    [string] $TimestampServer = 'http://timestamp.digicert.com',

    [string] $Website
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Windows = Split-Path -Parent $PSScriptRoot
if (-not $OutDir) { $OutDir = Join-Path $Windows 'artifacts' }
if (-not $Version) {
    if ($env:GITHUB_REF_NAME -match '^v(\d+\.\d+\.\d+.*)$') {
        $Version = $Matches[1]
    }
    else {
        $Version = (Select-Xml -Path (Join-Path $Windows 'Directory.Build.props') -XPath '//Version').Node.InnerText
    }
}
# Windows file versions have four numbers: 0.0.7 and 0.0.7-beta.1 give 0.0.7.0.
$numbers = @((($Version -split '[-+]')[0]).Split('.') | Select-Object -First 3)
while ($numbers.Count -lt 3) { $numbers += '0' }
$FileVersion = "$($numbers -join '.').0"

$Publish = [ordered]@{
    x64   = Join-Path $Windows 'publish\win-x64\Jellyfesh'
    arm64 = Join-Path $Windows 'publish\win-arm64\Jellyfesh'
}
foreach ($arch in $Publish.Keys) {
    if (-not (Test-Path (Join-Path $Publish[$arch] 'Jellyfesh.exe'))) {
        throw "There is no $arch build in $($Publish[$arch]): run scripts/build.ps1 for both architectures first"
    }
}

# Inno Setup's command-line compiler: on the PATH, else the newest "Inno Setup *" folder.
function Find-Iscc {
    $command = Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    foreach ($base in @(${env:ProgramFiles(x86)}, $env:ProgramFiles, (Join-Path $env:LOCALAPPDATA 'Programs'))) {
        if (-not $base -or -not (Test-Path $base)) { continue }
        $found = Get-ChildItem $base -Directory -Filter 'Inno Setup*' -ErrorAction SilentlyContinue |
            Sort-Object Name -Descending |
            ForEach-Object { Join-Path $_.FullName 'ISCC.exe' } |
            Where-Object { Test-Path $_ } |
            Select-Object -First 1
        if ($found) { return $found }
    }
    throw 'Inno Setup 6.3 or later is needed (ISCC.exe was not found): winget install JRSoftware.InnoSetup'
}

# signtool.exe of the newest Windows SDK installed, for x64 hosts.
function Find-Signtool {
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $found = @(Get-ChildItem $kits -Recurse -Filter 'signtool.exe' -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\bin\\(10\.[0-9.]+)\\x64\\' } |
        Sort-Object { [version]($_.FullName -replace '^.*\\bin\\(10\.[0-9.]+)\\x64\\.*$', '$1') } -Descending)
    if ($found.Count -eq 0) { throw "signtool.exe is not in ${kits}; install the Windows SDK" }
    return $found[0].FullName
}

$iscc = Find-Iscc
Write-Host "Inno Setup $((Get-Item $iscc).VersionInfo.ProductVersion) ($iscc)"
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$name = "Jellyfesh-$Version-Windows-Setup"
$arguments = @(
    "/DJfVersion=$Version",
    "/DJfFileVersion=$FileVersion",
    "/DJfX64=$($Publish.x64)",
    "/DJfArm64=$($Publish.arm64)",
    "/O$OutDir",
    "/F$name")
if ($Website) { $arguments += "/DJfWebsite=$Website" }

$thumbprint = $null
try {
    if ($SigningCertificate) {
        if (-not (Test-Path $SigningCertificate)) { throw "-SigningCertificate: $SigningCertificate does not exist" }
        if (-not $env:WINDOWS_SIGNING_PASSWORD) { throw '-SigningCertificate needs its password in WINDOWS_SIGNING_PASSWORD' }
        $signtool = Find-Signtool
        $flags = [System.Security.Cryptography.X509Certificates.X509KeyStorageFlags]'UserKeySet, PersistKeySet'
        $certificate = [System.Security.Cryptography.X509Certificates.X509Certificate2]::new(
            (Resolve-Path $SigningCertificate).Path, $env:WINDOWS_SIGNING_PASSWORD, $flags)
        $store = [System.Security.Cryptography.X509Certificates.X509Store]::new('My', 'CurrentUser')
        $store.Open('ReadWrite')
        $store.Add($certificate)
        $store.Close()
        $thumbprint = $certificate.Thumbprint
        # $q and $f are Inno Setup's: a quote, and the file to sign.
        $arguments += '/DJfSign'
        $arguments += ('/Sjellyfesh=$q{0}$q sign /sha1 {1} /fd SHA256 /td SHA256 /tr {2} $f' -f $signtool, $thumbprint, $TimestampServer)
    }
    Write-Host "> ISCC $(($arguments | Where-Object { $_ -notlike '/S*' }) -join ' ') installer\Jellyfesh.iss" -ForegroundColor DarkGray
    & $iscc @arguments (Join-Path $Windows 'installer\Jellyfesh.iss')
    if ($LASTEXITCODE -ne 0) { throw "ISCC exited with $LASTEXITCODE" }
}
finally {
    if ($thumbprint) {
        Remove-Item -Path "Cert:\CurrentUser\My\$thumbprint" -DeleteKey -ErrorAction SilentlyContinue
    }
}

$setup = Join-Path $OutDir "$name.exe"
if (-not (Test-Path $setup)) { throw "ISCC wrote no $setup" }
Write-Host "Wrote $setup ($([math]::Round((Get-Item $setup).Length / 1MB, 1)) MB)"
