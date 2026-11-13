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
  5. a zip per architecture with its SHA-256, and optionally an unsigned MSIX for later.

Needs: Rust (rustup, with the MSVC build tools and CMake for libopus), the .NET 10 SDK,
Python 3 for the notices in CI. CARGO_TARGET_DIR is respected.

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

    [switch] $Msix
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

# ---- the official plugins shipped inside the app ----
$plugins = @(Get-ChildItem (Join-Path $Root 'dist\plugins') -Filter '*.jfpkg' -ErrorAction SilentlyContinue)
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
        Invoke-Native python @((Join-Path $Root 'tools\licenses\generate.py'), 'jf-ffi', 'jf-subsonic', 'jf-jellyfin')
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
}

# Leave the host's library in place for local runs from Visual Studio.
if (-not $SkipNative) {
    $hostArch = if ($hostTriple -like 'aarch64*') { 'arm64' } else { 'x64' }
    if ($Architectures -contains $hostArch) {
        Copy-Item (Get-NativeLibrary $hostArch $hostTriple) (Join-Path $nativeFolder 'jf_ffi.dll') -Force
    }
}
