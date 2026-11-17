<#
.SYNOPSIS
Builds the Windows app of Jellyfesh: the core's FFI library for x64 and arm64, the C# binding and
the design tokens, then an unpackaged, self-contained publish per architecture, zipped.

.DESCRIPTION
This is what CI runs on a windows-2025 runner (docs/windows.md). Steps:
  1. cargo xtask bindings --lang csharp --release: the Jellyfesh.Ffi C# library and the host's
     jf_ffi.dll into platforms/windows/Generated/;
  2. cargo xtask tokens --target csharp: Generated/Tokens.cs;
  3. cargo build -p jf-ffi --release --target <triple> for each other architecture;
  4. dotnet publish of Jellyfesh.App per architecture (Windows App SDK and .NET in the folder, so
     nothing needs installing, ADR-0016), with that architecture's jf_ffi.dll and the official
     plugins from dist/plugins (made by plugins/release.sh; skipped with a warning when absent);
  5. a zip per architecture with its SHA-256, and optionally an unsigned MSIX for later;
  6. with -StorePackage, the Microsoft Store upload package: an MSIX per architecture with the
     Partner Center identity, joined into one .msixbundle (unsigned: the Store signs it).

Edition and distribution (ADR-0021, docs/github.md): JF_EDITION (community, the default, or
commercial) and JF_DISTRIBUTION (source, the default, or direct) from the environment become MSBuild
properties (Directory.Build.props); the commercial edition bundles no unofficial community plugins.
The Store package is always the microsoft-store distribution.

Needs: Rust (rustup, with the MSVC build tools and CMake for libopus), the .NET 10 SDK,
Python 3 for the notices in CI, and for -SigningCertificate and -StorePackage the Windows SDK
(signtool, makeappx). CARGO_TARGET_DIR is respected.

.PARAMETER Architectures
x64, arm64 or both (the default).

.PARAMETER Configuration
Release (the default) or Debug.

.PARAMETER Version
The app version for file names and assemblies. Default: the tag in GITHUB_REF_NAME (v0.0.4 gives
0.0.4), else the Version in Directory.Build.props.

.PARAMETER OutDir
Where the zips go. Default: platforms/windows/artifacts.

.PARAMETER SkipNative
Use Generated/ and the native libraries already built (fast app-only rebuilds).

.PARAMETER Msix
Also make an unsigned MSIX package per architecture (the packaging option for later, ADR-0016).

.PARAMETER SigningCertificate
A .pfx code-signing certificate: Jellyfesh's own executables and libraries in each zip are signed
with it (SHA-256, with a timestamp from -TimestampServer). Its password is read from the
WINDOWS_SIGNING_PASSWORD environment variable, never from the command line.

.PARAMETER TimestampServer
The RFC 3161 timestamp server for -SigningCertificate. Default: http://timestamp.digicert.com.

.PARAMETER StorePackage
Also make the Microsoft Store upload package, Jellyfesh-<version>-microsoft-store.msixbundle. The
Partner Center identity (Product identity) comes from the environment: MS_STORE_IDENTITY_NAME
(Package/Identity/Name), MS_STORE_PUBLISHER (Package/Identity/Publisher, "CN=…") and
MS_STORE_PUBLISHER_NAME (Package/Properties/PublisherDisplayName).
#>
[CmdletBinding()]
param(
    [ValidateSet('x64', 'arm64')]
    [string[]] $Architectures = @('x64', 'arm64'),

    [ValidateSet('Release', 'Debug')]
    [string] $Configuration = 'Release',

    [string] $Version,

    [string] $OutDir,

    [switch] $SkipNative,

    [switch] $Msix,

    [string] $SigningCertificate,

    [string] $TimestampServer = 'http://timestamp.digicert.com',

    [switch] $StorePackage
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$Windows = Split-Path -Parent $PSScriptRoot
$Root = (Resolve-Path (Join-Path $Windows '..\..')).Path
$AppProject = Join-Path $Windows 'src\Jellyfesh.App\Jellyfesh.App.csproj'
$Generated = Join-Path $Windows 'Generated'
if (-not $OutDir) { $OutDir = Join-Path $Windows 'artifacts' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$env:CARGO_INCREMENTAL = '0'

$Triples = @{ x64 = 'x86_64-pc-windows-msvc'; arm64 = 'aarch64-pc-windows-msvc' }
$Rids = @{ x64 = 'win-x64'; arm64 = 'win-arm64' }
$Platforms = @{ x64 = 'x64'; arm64 = 'ARM64' }

function Invoke-Native {
    param([Parameter(Mandatory)] [string] $File, [string[]] $Arguments = @())
    Write-Host "> $File $($Arguments -join ' ')" -ForegroundColor DarkGray
    & $File @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$File exited with $LASTEXITCODE" }
}

# A tool of the newest Windows 10/11 SDK installed (signtool.exe, makeappx.exe), for x64 hosts.
function Find-SdkTool([string] $Name) {
    $kits = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $found = @(Get-ChildItem $kits -Recurse -Filter $Name -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -match '\\bin\\(10\.[0-9.]+)\\x64\\' } |
        Sort-Object { [version]($_.FullName -replace '^.*\\bin\\(10\.[0-9.]+)\\x64\\.*$', '$1') } -Descending)
    if ($found.Count -eq 0) { throw "$Name is not in ${kits}; install the Windows SDK" }
    return $found[0].FullName
}

# Signs files with -SigningCertificate. Not through Invoke-Native, which would print the password.
function Invoke-Sign([string[]] $Files) {
    $arguments = @('sign', '/fd', 'SHA256', '/td', 'SHA256', '/tr', $TimestampServer,
        '/f', $SigningCertificate, '/p', $env:WINDOWS_SIGNING_PASSWORD) + $Files
    Write-Host "> signtool sign /fd SHA256 /td SHA256 /tr $TimestampServer /f <certificate> $($Files -join ' ')" -ForegroundColor DarkGray
    & $script:Signtool @arguments
    if ($LASTEXITCODE -ne 0) { throw "signtool exited with $LASTEXITCODE" }
}

function Get-HostTriple {
    $line = & rustc -vV | Select-String '^host: (.+)$'
    if (-not $line) { throw 'rustc -vV did not name the host' }
    return $line.Matches[0].Groups[1].Value.Trim()
}

function Get-NativeLibrary([string] $Arch, [string] $HostTriple) {
    $triple = $Triples[$Arch]
    $hostBuild = Join-Path $TargetDir 'release\jf_ffi.dll'
    $crossBuild = Join-Path $TargetDir "$triple\release\jf_ffi.dll"
    if ($triple -eq $HostTriple -and (Test-Path $hostBuild)) { return $hostBuild }
    if (Test-Path $crossBuild) { return $crossBuild }
    throw "jf_ffi.dll for $triple was not built (looked in $crossBuild)"
}

if (-not $Version) {
    if ($env:GITHUB_REF_NAME -match '^v(\d+\.\d+\.\d+.*)$') {
        $Version = $Matches[1]
    }
    else {
        $Version = (Select-Xml -Path (Join-Path $Windows 'Directory.Build.props') -XPath '//Version').Node.InnerText
    }
}
Write-Host "Jellyfesh $Version for Windows ($($Architectures -join ', '), $Configuration)"

$Edition = if ($env:JF_EDITION) { $env:JF_EDITION } else { 'community' }
if ($Edition -notin @('community', 'commercial')) { throw "JF_EDITION must be community or commercial, not '$Edition'" }
$Distribution = if ($env:JF_DISTRIBUTION) { $env:JF_DISTRIBUTION } else { 'source' }
Write-Host "Edition $Edition, distribution $Distribution$(if ($StorePackage) { ', and the Microsoft Store package' })"

if ($SigningCertificate) {
    if (-not (Test-Path $SigningCertificate)) { throw "-SigningCertificate: $SigningCertificate does not exist" }
    if (-not $env:WINDOWS_SIGNING_PASSWORD) { throw '-SigningCertificate needs its password in WINDOWS_SIGNING_PASSWORD' }
    $script:Signtool = Find-SdkTool 'signtool.exe'
}
if ($StorePackage) {
    foreach ($name in 'MS_STORE_IDENTITY_NAME', 'MS_STORE_PUBLISHER', 'MS_STORE_PUBLISHER_NAME') {
        if (-not [Environment]::GetEnvironmentVariable($name)) { throw "-StorePackage needs $name (the Partner Center identity)" }
    }
    $Makeappx = Find-SdkTool 'makeappx.exe'
    # The Store's package versions have four parts, the last 0: 0.0.7 and 0.0.7-beta.1 give 0.0.7.0.
    $numbers = @((($Version -split '[-+]')[0]).Split('.') | Select-Object -First 3)
    while ($numbers.Count -lt 3) { $numbers += '0' }
    $StoreVersion = "$($numbers -join '.').0"
    # The manifest with the Partner Center identity, for the Store build only (JfAppxManifest).
    [xml] $storeManifestXml = Get-Content (Join-Path $Windows 'src\Jellyfesh.App\Package.appxmanifest') -Raw
    $ns = New-Object System.Xml.XmlNamespaceManager($storeManifestXml.NameTable)
    $ns.AddNamespace('m', 'http://schemas.microsoft.com/appx/manifest/foundation/windows10')
    $identity = $storeManifestXml.SelectSingleNode('/m:Package/m:Identity', $ns)
    $identity.SetAttribute('Name', $env:MS_STORE_IDENTITY_NAME)
    $identity.SetAttribute('Publisher', $env:MS_STORE_PUBLISHER)
    $identity.SetAttribute('Version', $StoreVersion)
    $storeManifestXml.SelectSingleNode('/m:Package/m:Properties/m:PublisherDisplayName', $ns).InnerText = $env:MS_STORE_PUBLISHER_NAME
    New-Item -ItemType Directory -Force -Path $Generated | Out-Null
    $StoreManifest = Join-Path $Generated 'Package.store.appxmanifest'
    $storeManifestXml.Save($StoreManifest)
    $StoreStage = Join-Path $OutDir 'store'
    if (Test-Path $StoreStage) { Remove-Item $StoreStage -Recurse -Force }
    New-Item -ItemType Directory -Force -Path (Join-Path $StoreStage 'bundle') | Out-Null
}

# ---- 1 to 3: the core's library, the C# binding and the tokens ----
$hostTriple = Get-HostTriple
if (-not $SkipNative) {
    Push-Location $Root
    try {
        foreach ($arch in $Architectures) {
            Invoke-Native rustup @('target', 'add', $Triples[$arch])
        }
        Invoke-Native cargo @('xtask', 'bindings', '--lang', 'csharp', '--release')
        Invoke-Native cargo @('xtask', 'tokens', '--target', 'csharp')
        foreach ($arch in $Architectures) {
            if ($Triples[$arch] -ne $hostTriple) {
                Invoke-Native cargo @('build', '--package', 'jf-ffi', '--release', '--target', $Triples[$arch])
            }
        }
    }
    finally {
        Pop-Location
    }
}
foreach ($required in @('Jellyfesh.Ffi\Jellyfesh.Ffi.csproj', 'Tokens.cs')) {
    if (-not (Test-Path (Join-Path $Generated $required))) {
        throw "Generated\$required is missing: run without -SkipNative first"
    }
}

# ---- the official plugins shipped inside the app (the project leaves out the community ones in the commercial edition) ----
$plugins = @(Get-ChildItem (Join-Path $Root 'dist\plugins') -Filter '*.jfpkg' -ErrorAction SilentlyContinue |
    Where-Object { $Edition -ne 'commercial' -or $_.Name -notlike 'community.unofficial.*' })
if ($plugins.Count -gt 0) {
    Write-Host "Bundling $($plugins.Count) official plugins: $(($plugins | ForEach-Object Name) -join ', ')"
}
else {
    Write-Warning 'No official plugins in dist\plugins (run plugins/release.sh or download the "plugins" artifact first). The app is built without them and starts with no sources installed.'
}

# ---- third-party notices: the Rust crates in jf_ffi.dll and the bundled plugins, then the .NET packages ----
$notices = Join-Path $Root 'THIRD_PARTY_NOTICES.md'
if ($env:CI -eq 'true' -and -not $SkipNative) {
    Push-Location $Root
    try {
        $crates = @('jf-ffi', 'jf-subsonic', 'jf-jellyfin', 'jf-qqmusic', 'jf-spotify', 'jf-netease')
        if ($Edition -ne 'commercial') { $crates += @('jf-community-netease', 'jf-community-qqmusic') }
        Invoke-Native python (@((Join-Path $Root 'tools\licenses\generate.py')) + $crates)
    }
    finally {
        Pop-Location
    }
}
$dotnetNotices = @'
# .NET components in the Windows app

Besides the Rust crates listed in THIRD_PARTY_NOTICES.md, the Windows app ships:

| Component | Licence |
| --- | --- |
| .NET runtime (self-contained) | MIT, https://github.com/dotnet/runtime/blob/main/LICENSE.TXT |
| Windows App SDK and WinUI 3 (self-contained runtime) | MIT and the Windows App SDK runtime licence, https://www.nuget.org/packages/Microsoft.WindowsAppSDK |
| CommunityToolkit.Mvvm | MIT, https://github.com/CommunityToolkit/dotnet/blob/main/License.md |
| C#/WinRT runtime (WinRT.Runtime) | MIT, https://github.com/microsoft/CsWinRT/blob/master/LICENSE |
'@

# ---- 4 and 5: publish, zip, checksum ----
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$nativeFolder = Join-Path $Generated 'native'
New-Item -ItemType Directory -Force -Path $nativeFolder | Out-Null
foreach ($arch in $Architectures) {
    $rid = $Rids[$arch]
    Copy-Item (Get-NativeLibrary $arch $hostTriple) (Join-Path $nativeFolder 'jf_ffi.dll') -Force
    $stage = Join-Path $Windows "publish\$rid"
    $folder = Join-Path $stage 'Jellyfesh'
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }

    Invoke-Native dotnet @(
        'publish', $AppProject,
        '--configuration', $Configuration,
        '--runtime', $rid,
        '--self-contained', 'true',
        '--output', $folder,
        "-p:Platform=$($Platforms[$arch])",
        "-p:Version=$Version",
        '-p:WindowsPackageType=None',
        '-nologo')

    if (Test-Path $notices) { Copy-Item $notices (Join-Path $folder 'THIRD_PARTY_NOTICES.md') }
    Set-Content -Path (Join-Path $folder 'THIRD_PARTY_NOTICES_DOTNET.md') -Value $dotnetNotices -Encoding utf8

    if ($SigningCertificate) {
        # Jellyfesh's own files; the .NET and Windows App SDK files come signed by Microsoft.
        $own = @(Get-ChildItem $folder -File | Where-Object { $_.Name -like 'Jellyfesh*.exe' -or $_.Name -like 'Jellyfesh*.dll' -or $_.Name -eq 'jf_ffi.dll' } |
            ForEach-Object FullName)
        Invoke-Sign $own
        & $script:Signtool verify /pa (Join-Path $folder 'Jellyfesh.exe')
        if ($LASTEXITCODE -ne 0) { Write-Warning 'The signature does not verify against the trusted roots (a test certificate?)' }
    }

    $zip = Join-Path $OutDir "jellyfesh-$Version-windows-$arch.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path $folder -DestinationPath $zip
    $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    Set-Content -Path "$zip.sha256" -Value "$hash  $(Split-Path $zip -Leaf)" -Encoding ascii -NoNewline
    Write-Host "Wrote $zip"

    if ($Msix) {
        $packageDir = Join-Path $OutDir "msix-$arch\"
        Invoke-Native dotnet @(
            'publish', $AppProject,
            '--configuration', $Configuration,
            '--runtime', $rid,
            "-p:Platform=$($Platforms[$arch])",
            "-p:Version=$Version",
            '-p:WindowsPackageType=MSIX',
            '-p:GenerateAppxPackageOnBuild=true',
            '-p:AppxPackageSigningEnabled=false',
            "-p:AppxPackageDir=$packageDir",
            '-nologo')
        Write-Host "Wrote an unsigned MSIX to $packageDir"
    }

    if ($StorePackage) {
        $packageDir = Join-Path $StoreStage "msix-$arch\"
        Invoke-Native dotnet @(
            'publish', $AppProject,
            '--configuration', $Configuration,
            '--runtime', $rid,
            "-p:Platform=$($Platforms[$arch])",
            "-p:Version=$Version",
            '-p:WindowsPackageType=MSIX',
            '-p:GenerateAppxPackageOnBuild=true',
            '-p:AppxPackageSigningEnabled=false',
            '-p:AppxBundle=Never',
            "-p:AppxPackageDir=$packageDir",
            "-p:JfAppxManifest=$StoreManifest",
            '-p:JfDistribution=microsoft-store',
            '-nologo')
        $msix = @(Get-ChildItem $packageDir -Recurse -Filter '*.msix')
        if ($msix.Count -ne 1) { throw "expected one .msix in $packageDir, found $($msix.Count)" }
        Copy-Item $msix[0].FullName (Join-Path $StoreStage "bundle\Jellyfesh_$($StoreVersion)_$arch.msix")
    }
}

if ($StorePackage) {
    $bundle = Join-Path $OutDir "Jellyfesh-$Version-microsoft-store.msixbundle"
    if (Test-Path $bundle) { Remove-Item $bundle -Force }
    Invoke-Native $Makeappx @('bundle', '/d', (Join-Path $StoreStage 'bundle'), '/p', $bundle, '/bv', $StoreVersion, '/o')
    Write-Host "Wrote $bundle for Partner Center ($env:MS_STORE_IDENTITY_NAME $StoreVersion)"
}

# Leave the host's library in place for local runs from Visual Studio.
if (-not $SkipNative) {
    $hostArch = if ($hostTriple -like 'aarch64*') { 'arm64' } else { 'x64' }
    if ($Architectures -contains $hostArch) {
        Copy-Item (Get-NativeLibrary $hostArch $hostTriple) (Join-Path $nativeFolder 'jf_ffi.dll') -Force
    }
}
